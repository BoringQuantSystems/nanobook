//! Order submission, fill monitoring, rate limiting, and cancellation.
//!
//! # Cancel Reject Race Handling (F2 Failure Mode)
//!
//! This module implements handling for the F2 failure mode: cancel reject race with fill reconciliation.
//!
//! When a cancel request races against an in-flight fill:
//! 1. Order fills on the market before cancel reaches the broker
//! 2. Broker rejects the cancel request because the order is already complete
//! 3. System must reconcile that the order is filled despite the cancel rejection
//!
//! The implementation includes:
//! - `BrokerError::CancelReject` variant to capture rejection details (order_id, reason)
//! - `cancel_order` returns `Result<(), BrokerError>` to surface rejections to callers
//! - Audit logging at info level for all cancel attempts and rejections
//! - `reconcile_filled_order` function to verify order state when cancel is rejected
//! - Integration with `execute_limit_order` to handle timeout cancellations with reconciliation
//!
//! # Disconnect During Order Execution (F3 Failure Mode)
//!
//! This module implements handling for the F3 failure mode: partial fill followed by disconnect.
//!
//! When a connection is lost during order execution:
//! 1. Order may have been partially filled before disconnect
//! 2. The subscription loop terminates (explicitly or silently)
//! 3. System must detect the disconnect and preserve partial fill state
//! 4. On reconnect, system queries positions to reconcile actual fill state
//!
//! The implementation includes:
//! - `BrokerError::ConnectionLost` variant to capture disconnect with partial fill state (order_id, filled_quantity)
//! - `execute_limit_order` detects disconnect errors (IBKR error codes 1100, 1101, 1102)
//! - `execute_limit_order` detects silent disconnects (subscription terminates early without timeout)
//! - Audit logging at info level for disconnect events
//! - `reconcile_partial_fill` function to reconcile state using ground truth from positions
//! - `IbkrClient::reconnect` method to re-establish connection and query positions
//!
//! # Important: No Double-Submit on Reconnect
//!
//! When a disconnect occurs with a partial fill, the remainder is **NOT** automatically resubmitted.
//! This is deliberate because:
//! - We cannot guarantee the original order is still active at the broker
//! - The broker may have cancelled the order during the disconnect
//! - Resubmitting could lead to duplicate positions
//! - Manual review is required to determine the correct action
//!
//! The `reconcile_partial_fill` function returns the reconciled state but does not resubmit.
//! It logs "remainder NOT resubmitted" to make this explicit in the audit trail.
//!
//! # Audit Log Format
//!
//! Cancel-related audit logs use the "AUDIT:" prefix for easy filtering:
//! - `AUDIT: Cancel attempt for order {id} at {timestamp}` - before sending cancel request
//! - `AUDIT: Cancel rejected for order {id} - reason: {reason}` - when broker rejects cancel
//! - `AUDIT: Cancel confirmed for order {id} (status: {status})` - when cancel accepted
//! - `AUDIT: Cancel moot for order {id} (already completed)` - when order already filled/cancelled
//! - `AUDIT: Reconciling order {id} after cancel reject - reason: {reason}` - reconciliation attempt
//! - `AUDIT: Order {id} reconciled as FILLED (cancel rejected due to fill race)` - reconciliation success
//! - `AUDIT: Order {id} state uncertain after cancel reject - manual review recommended` - uncertain state
//!
//! Disconnect-related audit logs:
//! - `AUDIT: Connection lost during order {id} execution (code {code}, filled={filled})` - explicit disconnect error
//! - `AUDIT: Subscription terminated early for order {id} (filled={filled}) - likely silent disconnect` - silent disconnect
//! - `AUDIT: Reconciling order {id} after disconnect - symbol={symbol}, pre_disconnect_filled={pre}, expected_total={total}` - reconciliation attempt
//! - `AUDIT: Order {id} reconciled as PARTIAL_FILL (filled={filled}, remainder={remainder}) - remainder NOT resubmitted` - reconciliation success
//! - `AUDIT: Order {id} no additional fills detected during disconnect (filled={filled})` - no additional fills
//! - `AUDIT: Order {id} position not found after disconnect - manual review required` - position missing
//!
//! These logs enable post-mortem analysis of cancel/fill race conditions and disconnect
//! reconciliation, and verification that the remainder was not resubmitted.

use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use ibapi::client::blocking::Client;
use ibapi::contracts::Contract;
#[cfg(not(feature = "strict-market-reject"))]
use ibapi::orders::order_builder::market_order;
use ibapi::orders::order_builder::{limit_order, stop};
use ibapi::orders::{
    Action as IbAction, CancelOrder, Order, OrderData, OrderStatusKind, PlaceOrder,
};
use ibapi::subscriptions::SubscriptionItem;
use log::{debug, info, warn};

use crate::error::BrokerError;
use crate::types::*;

pub fn map_ibkr_order_status(status: &str) -> OrderState {
    match status {
        "Submitted" | "PreSubmitted" | "PendingSubmit" | "ApiPending" => OrderState::Submitted,
        "PartiallyFilled" => OrderState::PartiallyFilled,
        "Filled" => OrderState::Filled,
        "Cancelled" | "ApiCancelled" | "PendingCancel" => OrderState::Cancelled,
        "Inactive" => OrderState::Rejected,
        _ => OrderState::Submitted,
    }
}

fn rounded_non_negative_u64(value: f64, field: &'static str) -> Result<u64, BrokerError> {
    if !value.is_finite() {
        return Err(BrokerError::Order(format!(
            "{field} is not finite: {value}"
        )));
    }
    Ok(value.max(0.0).round() as u64)
}

pub fn broker_order_status_from_ibkr_parts(
    order_id: i32,
    status: &str,
    filled: f64,
    remaining: f64,
    average_fill_price: f64,
) -> Result<BrokerOrderStatus, BrokerError> {
    let filled_quantity = rounded_non_negative_u64(filled, "ibkr order.filled")?;
    let remaining_quantity = rounded_non_negative_u64(remaining, "ibkr order.remaining")?;
    let mapped = if filled_quantity > 0 && remaining_quantity > 0 {
        OrderState::PartiallyFilled
    } else {
        map_ibkr_order_status(status)
    };

    Ok(BrokerOrderStatus {
        id: OrderId(order_id as u64),
        status: mapped,
        filled_quantity,
        remaining_quantity,
        avg_fill_price_cents: f64_cents_checked(
            average_fill_price,
            "ibkr order.average_fill_price",
        )?,
    })
}

pub fn broker_order_status_from_order_data(
    order_data: &OrderData,
) -> Result<BrokerOrderStatus, BrokerError> {
    broker_order_status_from_ibkr_parts(
        order_data.order_id,
        order_data.order_state.status.as_str(),
        0.0,
        order_data.order.total_quantity,
        0.0,
    )
}

/// Deduplication key for order-status callbacks.
///
/// TWS (Trader Workstation) may send duplicate OrderStatus callbacks for the
/// same fill event due to network retries, internal TWS state synchronization,
/// or other implementation details. Without deduplication, these duplicates
/// would cause position updates to be applied multiple times, leading to
/// incorrect position tracking.
///
/// We deduplicate based on the combination of:
/// - `order_id`: The broker-assigned order identifier
/// - `status`: The order status string (e.g., "Filled", "Submitted")
/// - `filled_quantity`: The quantity filled (rounded to integer for hashing)
///
/// This ensures that each unique fill event is processed exactly once,
/// regardless of how many duplicate callbacks TWS sends.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderCallbackKey {
    pub order_id: i32,
    pub status: String,
    pub filled_quantity: i64, // Converted from f64 for hashing
}

/// Deduplication cache type for order-status callbacks.
///
/// The cache maps each unique callback to the timestamp when it was first
/// seen. Entries are automatically cleaned up after a TTL (time-to-live) of
/// 5 minutes to prevent unbounded memory growth.
///
/// The TTL of 5 minutes is chosen to be:
/// - Long enough to cover the typical window for duplicate callbacks
/// - Short enough to prevent the cache from growing indefinitely
/// - Sufficient for normal order execution flows (orders typically fill within seconds)
pub type CallbackDedupCache = Mutex<HashMap<OrderCallbackKey, Instant>>;

/// Result of a single order execution.
#[derive(Debug, Clone)]
pub struct OrderResult {
    pub symbol: nanobook::Symbol,
    pub order_id: i32,
    pub filled_shares: i64,
    pub avg_fill_price: f64,
    pub commission: f64,
    pub status: OrderOutcome,
}

/// How an order ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderOutcome {
    Filled,
    PartialFill,
    Cancelled,
    Failed,
}

/// Encode a `BrokerOrder` into the `(limit_price_f64, qty_f64)` pair used by
/// the quote-bounded fallback path.
///
/// Investigation note: `ibapi` 2.7 and 2.11 both expose true market orders via
/// `order_builder::market_order` (`order_type = "MKT"`), and
/// `OrderType::Market` does not require a limit price. Live IBKR market
/// submissions therefore use true market orders. This encoder remains the
/// bounded aggressive-limit helper for callers that explicitly choose a
/// quote-bounded fallback.
///
/// # Errors
/// - `BrokerError::NoQuoteForMarketOrder` if a market order is encoded without
///   a cached NBBO quote.
/// - `BrokerError::MarketOrderRejected` if `strict-market-reject` is enabled.
pub fn encode_order(
    order: &BrokerOrder,
    best_quote: Option<&BestQuote>,
) -> Result<(f64, f64), BrokerError> {
    #[cfg(feature = "strict-market-reject")]
    let _ = best_quote;

    let quantity = order.quantity as f64;
    match order.order_type {
        #[cfg(feature = "strict-market-reject")]
        BrokerOrderType::Market => Err(BrokerError::MarketOrderRejected),

        #[cfg(not(feature = "strict-market-reject"))]
        BrokerOrderType::Market => {
            let quote = best_quote.ok_or_else(|| BrokerError::NoQuoteForMarketOrder {
                symbol: order.symbol.to_string(),
            })?;
            const SLIP_BPS: f64 = 50.0;
            let bps = SLIP_BPS / 10_000.0;
            let price = match order.side {
                BrokerSide::Buy => (quote.ask_cents as f64 / 100.0) * (1.0 + bps),
                BrokerSide::Sell => (quote.bid_cents as f64 / 100.0) * (1.0 - bps),
            };
            Ok((price, quantity))
        }

        BrokerOrderType::Limit(price) => Ok((price.0 as f64 / 100.0, quantity)),
        BrokerOrderType::Stop(price) => Ok((price.0 as f64 / 100.0, quantity)),
        BrokerOrderType::StopLimit { limit, .. } => Ok((limit.0 as f64 / 100.0, quantity)),
    }
}

fn encode_ibkr_order(
    order: &BrokerOrder,
    best_quote: Option<&BestQuote>,
) -> Result<Order, BrokerError> {
    let ib_action = match order.side {
        BrokerSide::Buy => IbAction::Buy,
        BrokerSide::Sell => IbAction::Sell,
    };

    match order.order_type {
        #[cfg(feature = "strict-market-reject")]
        BrokerOrderType::Market => Err(BrokerError::MarketOrderRejected),

        #[cfg(not(feature = "strict-market-reject"))]
        BrokerOrderType::Market => Ok(market_order(ib_action, order.quantity as f64)),

        BrokerOrderType::Limit(_) => {
            let (limit_price, quantity) = encode_order(order, best_quote)?;
            Ok(limit_order(ib_action, quantity, limit_price))
        }

        BrokerOrderType::Stop(stop_price) => Ok(stop(
            ib_action,
            order.quantity as f64,
            stop_price.0 as f64 / 100.0,
        )),

        BrokerOrderType::StopLimit {
            stop: stop_price,
            limit,
        } => Ok(stop_limit(
            ib_action,
            order.quantity as f64,
            limit.0 as f64 / 100.0,
            stop_price.0 as f64 / 100.0,
        )),
    }
}

/// Build a stop-limit order. `ibapi` 5 removed `order_builder::stop_limit`
/// (its two same-typed price arguments were easy to swap); this keeps the
/// same fields: `aux_price` is the stop trigger, `limit_price` the limit.
fn stop_limit(action: IbAction, quantity: f64, limit_price: f64, stop_price: f64) -> Order {
    Order {
        action,
        order_type: "STP LMT".to_owned(),
        total_quantity: quantity,
        limit_price: Some(limit_price),
        aux_price: Some(stop_price),
        ..Order::default()
    }
}

/// Submit an order via the IBKR API. Returns the broker-assigned OrderId.
pub fn submit_order(
    client: &Client,
    order: &BrokerOrder,
    best_quote: Option<&BestQuote>,
) -> Result<OrderId, BrokerError> {
    let contract = Contract::stock(order.symbol.as_str()).build();

    let mut ib_order = encode_ibkr_order(order, best_quote)?;

    if let Some(cid) = &order.client_order_id {
        ib_order.order_ref = cid.as_str().to_string();
    }

    let order_id = client
        .next_valid_order_id()
        .map_err(|e| BrokerError::Order(format!("failed to get order id: {e}")))?;

    match order.order_type {
        BrokerOrderType::Market => info!(
            "Submitting: {:?} {} {} @ MKT (id={})",
            order.side, order.quantity, order.symbol, order_id
        ),
        BrokerOrderType::Limit(price) => info!(
            "Submitting: {:?} {} {} @ ${:.2} (id={})",
            order.side,
            order.quantity,
            order.symbol,
            price.0 as f64 / 100.0,
            order_id
        ),
        BrokerOrderType::Stop(price) => info!(
            "Submitting: {:?} {} {} @ STP ${:.2} (id={})",
            order.side,
            order.quantity,
            order.symbol,
            price.0 as f64 / 100.0,
            order_id
        ),
        BrokerOrderType::StopLimit { stop, limit } => info!(
            "Submitting: {:?} {} {} @ STP ${:.2} LMT ${:.2} (id={})",
            order.side,
            order.quantity,
            order.symbol,
            stop.0 as f64 / 100.0,
            limit.0 as f64 / 100.0,
            order_id
        ),
    }

    let _subscription = client
        .place_order(order_id, &contract, &ib_order)
        .map_err(|e| BrokerError::Order(format!("failed to place order {order_id}: {e}")))?;

    Ok(OrderId(order_id as u64))
}

/// Execute a rebalance-style order: submit limit, poll for fill, cancel on timeout.
///
/// This is the higher-level function used by the rebalancer for order-by-order execution.
///
/// # Deduplication
///
/// The `dedup_cache` parameter enables deduplication of order-status callbacks to
/// handle TWS duplicate callbacks. When provided, the function will:
/// - Check each OrderStatus callback against the cache
/// - Skip processing of duplicate callbacks (logged at debug level)
/// - Record new callbacks in the cache with a timestamp
/// - Automatically clean up expired entries (TTL: 5 minutes) on each check
///
/// When `None`, no deduplication is performed. This is useful for:
/// - Testing scenarios where duplicates are not expected
/// - Environments where TWS behavior is known to be stable
///
/// # Why Deduplication is Needed
///
/// TWS (Trader Workstation) may send duplicate OrderStatus callbacks for the same
/// fill event. Without deduplication, these duplicates would cause:
/// - Position updates to be applied multiple times
/// - Incorrect position tracking
/// - Potential double-counting of fills
///
/// The deduplication logic ensures that each unique fill event (identified by
/// order_id + status + filled_quantity) is processed exactly once.
#[allow(clippy::too_many_arguments)]
pub fn execute_limit_order(
    client: &Client,
    symbol: nanobook::Symbol,
    side: BrokerSide,
    shares: i64,
    limit_price_cents: i64,
    client_order_id: Option<&ClientOrderId>,
    timeout: Duration,
    dedup_cache: Option<&CallbackDedupCache>,
) -> Result<OrderResult, BrokerError> {
    let contract = Contract::stock(symbol.as_str()).build();

    let ib_action = match side {
        BrokerSide::Buy => IbAction::Buy,
        BrokerSide::Sell => IbAction::Sell,
    };

    let limit_price = limit_price_cents as f64 / 100.0;
    let quantity = shares as f64;

    let mut ib_order = limit_order(ib_action, quantity, limit_price);
    if let Some(cid) = client_order_id {
        ib_order.order_ref = cid.as_str().to_string();
    }

    let order_id = client
        .next_valid_order_id()
        .map_err(|e| BrokerError::Order(format!("failed to get order id: {e}")))?;

    info!(
        "Submitting: {:?} {} {} @ ${:.2} (id={})",
        side, shares, symbol, limit_price, order_id
    );

    let subscription = client
        .place_order(order_id, &contract, &ib_order)
        .map_err(|e| BrokerError::Order(format!("failed to place order {order_id}: {e}")))?;

    let start = Instant::now();
    let mut filled = 0.0_f64;
    // Last average fill price IB reported (a positive number). Kept when a later
    // status carries none.
    let mut avg_price: Option<f64> = None;
    let mut commission = 0.0_f64;
    let mut final_status = OrderOutcome::Failed;
    // True once the loop saw a final status for the order: filled, cancelled,
    // cancel confirmed, or rejected by IB.
    let mut terminal = false;

    for item in subscription {
        if start.elapsed() > timeout {
            warn!("Order {order_id} timed out after {}s", timeout.as_secs());
            match cancel_order_outcome(client, order_id) {
                Ok(CancelOutcome::Cancelled) => {
                    final_status = if filled > 0.0 {
                        OrderOutcome::PartialFill
                    } else {
                        OrderOutcome::Cancelled
                    };
                    terminal = true;
                }
                Ok(CancelOutcome::Filled) => {
                    // The order filled before the cancel reached IB.
                    filled = filled.max(quantity);
                    final_status = OrderOutcome::Filled;
                    terminal = true;
                }
                Ok(CancelOutcome::Requested) => {
                    warn!("Cancel for order {order_id} sent but not confirmed - state unknown");
                }
                Err(BrokerError::CancelReject {
                    order_id: oid,
                    reason,
                }) => {
                    debug!("Cancel rejected (order may have filled): {reason}");
                    if let Ok(true) = reconcile_filled_order(oid, &reason) {
                        filled = filled.max(quantity);
                        final_status = OrderOutcome::Filled;
                        terminal = true;
                    }
                }
                Err(e) => {
                    warn!("Cancel failed: {e}");
                }
            }
            // Not terminal: the order may still be live. The check after the loop
            // reports it as an error instead of Cancelled.
            break;
        }

        let response = match item {
            Ok(SubscriptionItem::Data(response)) => response,
            Ok(SubscriptionItem::Notice(notice)) => {
                match classify_notice(notice.code) {
                    NoticeAction::ConnectionLost => {
                        return Err(connection_lost(order_id, filled, notice.code));
                    }
                    NoticeAction::Warn => {
                        warn!("Order {order_id} error {}: {}", notice.code, notice.message);
                    }
                    NoticeAction::Ignore => {}
                }
                continue;
            }
            Err(e) => {
                // A stream error ends the subscription. A connectivity notice is a
                // lost connection; anything else falls through to the early-end check below.
                warn!("Order {order_id} stream error: {e}");
                if let ibapi::Error::Notice(notice) = &e
                    && classify_notice(notice.code) == NoticeAction::ConnectionLost
                {
                    return Err(connection_lost(order_id, filled, notice.code));
                }
                if classify_stream_error(&e) == StreamEnd::Rejected && filled == 0.0 {
                    final_status = OrderOutcome::Failed;
                    terminal = true;
                }
                break;
            }
        };

        match response {
            PlaceOrder::OrderStatus(status) => {
                // Check for duplicate callbacks if dedup cache is provided
                if let Some(cache) = dedup_cache {
                    let key = OrderCallbackKey {
                        order_id,
                        status: status.status.as_str().to_owned(),
                        filled_quantity: status.filled.round() as i64,
                    };

                    let is_duplicate = {
                        let mut cache_guard = cache
                            .lock()
                            .map_err(|_| BrokerError::Other("dedup cache poisoned".into()))?;

                        // Clean up expired entries (TTL: 5 minutes)
                        let ttl = Duration::from_secs(300);
                        cache_guard.retain(|_, timestamp| timestamp.elapsed() < ttl);

                        // Check if this is a duplicate
                        let duplicate = cache_guard.contains_key(&key);

                        // Record this callback if not a duplicate
                        if !duplicate {
                            cache_guard.insert(key, Instant::now());
                        }

                        duplicate
                    };

                    if is_duplicate {
                        debug!(
                            "Skipping duplicate OrderStatus for order {}: status={}, filled={}",
                            order_id,
                            status.status.as_str(),
                            status.filled
                        );
                        continue; // Skip processing this duplicate
                    }
                }

                debug!(
                    "Order {order_id} status: {} filled={} remaining={}",
                    status.status.as_str(),
                    status.filled,
                    status.remaining
                );
                filled = status.filled;
                if let Some(price) = reported_fill_price(status.average_fill_price) {
                    avg_price = Some(price);
                }

                if status.status == OrderStatusKind::Filled {
                    final_status = OrderOutcome::Filled;
                    terminal = true;
                    break;
                } else if status.status == OrderStatusKind::Cancelled {
                    final_status = if filled > 0.0 {
                        OrderOutcome::PartialFill
                    } else {
                        OrderOutcome::Cancelled
                    };
                    terminal = true;
                    break;
                }
            }
            PlaceOrder::ExecutionData(exec) => {
                debug!(
                    "Execution: {} shares @ ${:.2}",
                    exec.execution.shares, exec.execution.price
                );
            }
            PlaceOrder::CommissionReport(comm) => {
                commission = comm.commission;
                debug!("Commission: ${:.4}", commission);
            }
            _ => {}
        }
    }

    // The status stream ended without a final status (dropped connection, stream
    // error, closed subscription). The order may still be live at IB, so report
    // it as an error and let the caller reconcile. Never report it as Failed.
    if let Some(err) = unfinished_order_error(order_id, filled, terminal) {
        return Err(err);
    }

    // Never publish a fill at price 0: with shares filled and no price from IB
    // the result is unknown, so report it for reconciliation.
    let Some(avg_price) = fill_price(filled, avg_price) else {
        warn!("AUDIT: Order {order_id} filled={filled} but IB sent no average fill price");
        return Err(BrokerError::ConnectionLost {
            order_id,
            filled_quantity: filled.round() as i64,
        });
    };

    let result = OrderResult {
        symbol,
        order_id,
        filled_shares: filled as i64,
        avg_fill_price: avg_price,
        commission,
        status: final_status,
    };

    info!(
        "Order {order_id}: {:?} -- filled {} @ ${:.2}",
        final_status, result.filled_shares, avg_price
    );

    Ok(result)
}

/// Cancel an order by ID.
///
/// Returns `Ok(())` if the cancel is accepted or the order is already cancelled/filled.
/// Returns `Err(BrokerError::CancelReject)` if the broker explicitly rejects the cancel request.
///
/// # Cancel Reject Race Condition
///
/// This can happen when a cancel races against an in-flight fill:
/// 1. Order is submitted and fills on the market
/// 2. Cancel request is sent (either due to timeout or explicit cancellation)
/// 3. Cancel reaches broker after the order has already filled
/// 4. Broker rejects the cancel because the order is already complete
///
/// In this scenario, the cancel rejection is not an error - it's expected behavior
/// when the order filled before the cancel could be processed. The reconciliation
/// logic (`reconcile_filled_order`) handles this by verifying the order state and
/// ensuring position tracking reflects the filled status.
///
/// # Audit Logging
///
/// This function logs audit events at info level for:
/// - Cancel attempts (before sending request) - format: "AUDIT: Cancel attempt for order {id} at {timestamp}"
/// - Cancel rejections (when broker rejects the cancel) - format: "AUDIT: Cancel rejected for order {id} - reason: {reason}"
/// - Cancel confirmations (when cancel is accepted) - format: "AUDIT: Cancel confirmed for order {id} (status: {status})"
/// - Cancel moot (when order already completed) - format: "AUDIT: Cancel moot for order {id} (already completed)"
///
/// These logs capture the order_id, timestamp, and reason to enable tracking of
/// cancel/fill race conditions and post-mortem analysis.
pub fn cancel_order(client: &Client, order_id: i32) -> Result<(), BrokerError> {
    cancel_order_outcome(client, order_id).map(|_| ())
}

/// How a cancel request ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CancelOutcome {
    /// IB confirmed the order is cancelled.
    Cancelled,
    /// The order filled before the cancel reached IB.
    Filled,
    /// The request was sent and the stream ended with no confirmation.
    Requested,
}

/// What a cancel notice or error message says about the order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CancelText {
    /// The order already filled: the cancel is moot and the order is a fill.
    AlreadyFilled,
    /// IB does not know the order: the cancel failed and the order state is unknown.
    NoSuchOrder,
    /// IB refused the cancel.
    CannotCancel,
    /// Nothing that decides the outcome.
    Other,
}

fn classify_cancel_text(message: &str) -> CancelText {
    let msg = message.to_lowercase();
    if msg.contains("already filled") || msg.contains("order completed") {
        CancelText::AlreadyFilled
    } else if msg.contains("no such order") {
        CancelText::NoSuchOrder
    } else if msg.contains("cannot cancel") {
        CancelText::CannotCancel
    } else {
        CancelText::Other
    }
}

/// Send a cancel and report how it ended. See [`cancel_order`].
fn cancel_order_outcome(client: &Client, order_id: i32) -> Result<CancelOutcome, BrokerError> {
    // Audit log: cancel attempt
    info!(
        "AUDIT: Cancel attempt for order {} at {:?}",
        order_id,
        std::time::SystemTime::now()
    );
    let subscription = match client.cancel_order(order_id, "") {
        Ok(subscription) => subscription,
        Err(e) => return cancel_failed(order_id, &e.to_string()),
    };
    for item in subscription {
        let response = match item {
            Ok(SubscriptionItem::Data(response)) => response,
            Ok(SubscriptionItem::Notice(notice)) => {
                debug!("Cancel notice for {order_id}: {}", notice.message);
                match classify_cancel_text(&notice.message) {
                    CancelText::AlreadyFilled => {
                        info!(
                            "AUDIT: Cancel moot for order {order_id} (already filled): {}",
                            notice.message
                        );
                        return Ok(CancelOutcome::Filled);
                    }
                    CancelText::NoSuchOrder => {
                        return cancel_failed(order_id, &notice.message);
                    }
                    CancelText::CannotCancel => {
                        info!(
                            "AUDIT: Cancel rejected for order {} - reason: {}",
                            order_id, notice.message
                        );
                        return Err(BrokerError::CancelReject {
                            order_id,
                            reason: notice.message,
                        });
                    }
                    CancelText::Other => continue,
                }
            }
            Err(e) => return cancel_failed(order_id, &e.to_string()),
        };
        match response {
            CancelOrder::OrderStatus(s) => {
                debug!("Cancel status for {order_id}: {}", s.status.as_str());
                if s.status == OrderStatusKind::Cancelled {
                    info!(
                        "AUDIT: Cancel confirmed for order {} (status: {})",
                        order_id,
                        s.status.as_str()
                    );
                    return Ok(CancelOutcome::Cancelled);
                }
                if s.status == OrderStatusKind::Filled {
                    info!(
                        "AUDIT: Cancel moot for order {} (status: {})",
                        order_id,
                        s.status.as_str()
                    );
                    return Ok(CancelOutcome::Filled);
                }
            }
        }
    }
    info!("AUDIT: Cancel request sent for order {}", order_id);
    Ok(CancelOutcome::Requested)
}

/// Map a failed cancel request to a result, by what its message says.
fn cancel_failed(order_id: i32, message: &str) -> Result<CancelOutcome, BrokerError> {
    warn!("Failed to cancel order {order_id}: {message}");
    match classify_cancel_text(message) {
        CancelText::AlreadyFilled => {
            info!("AUDIT: Cancel moot for order {order_id} (already filled)");
            Ok(CancelOutcome::Filled)
        }
        CancelText::CannotCancel => Err(BrokerError::CancelReject {
            order_id,
            reason: message.to_owned(),
        }),
        CancelText::NoSuchOrder | CancelText::Other => Err(BrokerError::Order(format!(
            "failed to cancel order {order_id}: {message}"
        ))),
    }
}

/// An average fill price IB reported, if it is a usable one. IB sends 0 or
/// nothing when there is no fill.
pub(crate) fn reported_fill_price(price: Option<f64>) -> Option<f64> {
    price.filter(|p| p.is_finite() && *p > 0.0)
}

/// The fill price to publish. `None` when shares filled and no price is known.
/// With no fill the price is 0 by definition.
pub(crate) fn fill_price(filled: f64, known: Option<f64>) -> Option<f64> {
    match known {
        Some(price) => Some(price),
        None if filled > 0.0 => None,
        None => Some(0.0),
    }
}

/// How an order status stream that carried an error ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamEnd {
    /// IB rejected the order (order-rejection notice): the order is not live.
    Rejected,
    /// Any other error: the order state is unknown.
    Lost,
}

fn classify_stream_error(e: &ibapi::Error) -> StreamEnd {
    match e {
        ibapi::Error::Notice(notice) if notice.is_order_rejection() => StreamEnd::Rejected,
        _ => StreamEnd::Lost,
    }
}

/// The error for an order whose status stream ended before a final status.
/// `None` when the loop saw a final status.
fn unfinished_order_error(order_id: i32, filled: f64, terminal: bool) -> Option<BrokerError> {
    if terminal {
        return None;
    }
    warn!(
        "AUDIT: Subscription terminated early for order {order_id} (filled={filled}) - state unknown"
    );
    Some(BrokerError::ConnectionLost {
        order_id,
        filled_quantity: filled.round() as i64,
    })
}

/// What the order loop does with a notice code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoticeAction {
    /// Informational: farm status, warnings, 1102.
    Ignore,
    /// Log a warning and keep waiting.
    Warn,
    /// The connection to IB was lost or its data was lost: stop and report it.
    ConnectionLost,
}

/// Classify an IB notice code.
///
/// 1100 (connectivity lost) and 1101 (restored, data lost) stop the order
/// loop. 1102 (restored, data kept) does not. The connection codes are checked
/// before the warn range because 1100 and 1101 fall below 2000.
fn classify_notice(code: i32) -> NoticeAction {
    match code {
        1100 | 1101 => NoticeAction::ConnectionLost,
        c if !(0..2000).contains(&c) => NoticeAction::Warn,
        _ => NoticeAction::Ignore,
    }
}

fn connection_lost(order_id: i32, filled: f64, code: i32) -> BrokerError {
    warn!(
        "AUDIT: Connection lost during order {order_id} execution (code {code}, filled={filled})"
    );
    BrokerError::ConnectionLost {
        order_id,
        filled_quantity: filled.round() as i64,
    }
}

/// Reconcile order state when cancel is rejected.
///
/// When a cancel is rejected (typically because the order filled before the cancel
/// reached the broker), this function verifies the order state and logs the
/// reconciliation event. This ensures that position state reflects the filled status
/// even when the cancel request failed.
///
/// # Reconciliation Strategy
///
/// Since IBKR does not provide a direct "get order status" API, this function
/// infers the order state from the rejection reason:
/// - If the reason contains "filled", "completed", or "executed" → order is filled
/// - Otherwise → order state is uncertain, manual review recommended
///
/// In a production system, this would be enhanced with:
/// 1. Querying current positions to verify the fill
/// 2. Checking order execution reports from the broker
/// 3. Verifying against local order tracking state
/// 4. Cross-referencing with trade confirmation messages
///
/// # Arguments
/// * `order_id` - The order ID that was rejected for cancellation
/// * `rejection_reason` - The reason the broker rejected the cancel
///
/// # Returns
/// * `Ok(true)` if the order is confirmed to be filled
/// * `Ok(false)` if the order state could not be confirmed
/// * `Err(BrokerError)` if reconciliation fails
///
/// # Audit Logging
///
/// Logs reconciliation events at info level:
/// - "AUDIT: Reconciling order {id} after cancel reject - reason: {reason}"
/// - "AUDIT: Order {id} reconciled as FILLED (cancel rejected due to fill race)"
/// - "AUDIT: Order {id} state uncertain after cancel reject - manual review recommended"
///
/// These logs enable tracking of race condition resolution and identifying
/// orders that may need manual review.
pub fn reconcile_filled_order(order_id: i32, rejection_reason: &str) -> Result<bool, BrokerError> {
    info!(
        "AUDIT: Reconciling order {} after cancel reject - reason: {}",
        order_id, rejection_reason
    );

    // In a real implementation, this would query the broker's order history
    // or current positions to verify the order state. For IBKR, we would:
    // 1. Query current positions to see if the position reflects the fill
    // 2. Check order execution reports
    // 3. Verify against our local order tracking state

    // For this implementation, we infer the order state from the rejection reason:
    // - If the reason mentions "filled" or "completed", the order is likely filled
    // - Otherwise, we cannot confirm the state
    let reason_lower = rejection_reason.to_lowercase();
    let is_filled = reason_lower.contains("filled")
        || reason_lower.contains("completed")
        || reason_lower.contains("executed");

    if is_filled {
        info!(
            "AUDIT: Order {} reconciled as FILLED (cancel rejected due to fill race)",
            order_id
        );
    } else {
        info!(
            "AUDIT: Order {} state uncertain after cancel reject - manual review recommended",
            order_id
        );
    }

    Ok(is_filled)
}

/// Reconcile partial fill after disconnect using ground truth from positions.
///
/// This function is called after a reconnect to detect partial fills that occurred
/// during the disconnect window. It compares the pre-disconnect filled quantity
/// against the current position to determine if additional shares were filled.
///
/// # Reconciliation Strategy
///
/// The reconciliation uses ground truth from IBKR's position API:
/// 1. Query current positions after reconnect
/// 2. Find the position for the order's symbol
/// 3. Compare current quantity against expected quantity
/// 4. If current > expected, a partial fill occurred during disconnect
/// 5. Update order state to PartialFill with the reconciled quantity
///
/// # Important: No Double-Submit
///
/// This function **does NOT** resubmit the remainder of the order. The remainder
/// is deliberately left unsubmitted because:
/// - We cannot guarantee the original order is still active at the broker
/// - Resubmitting could lead to duplicate positions
/// - The original order may have been cancelled by the broker during disconnect
/// - Manual review is required to determine the correct action
///
/// # Arguments
/// * `order_id` - The order ID that was being executed when disconnect occurred
/// * `symbol` - The symbol of the order
/// * `pre_disconnect_filled` - The filled quantity before disconnect
/// * `expected_total` - The total expected quantity (original order size)
/// * `current_positions` - Current positions from reconnect (ground truth)
/// * `side` - The order side (Buy/Sell)
///
/// # Returns
/// * `Ok(OrderResult)` - Reconciled order result with updated filled quantity
/// * `Err(BrokerError)` - If reconciliation fails
///
/// # Audit Logging
///
/// Logs reconciliation events at info level:
/// - "AUDIT: Reconciling order {id} after disconnect - symbol={symbol}, pre_disconnect_filled={pre}, expected_total={total}"
/// - "AUDIT: Order {id} reconciled as PARTIAL_FILL (filled={filled}, remainder={remainder}) - remainder NOT resubmitted"
/// - "AUDIT: Order {id} no additional fills detected during disconnect (filled={filled})"
/// - "AUDIT: Order {id} position not found after disconnect - manual review required"
///
/// These logs enable tracking of disconnect reconciliation and verification
/// that the remainder was not resubmitted.
pub fn reconcile_partial_fill(
    order_id: i32,
    symbol: nanobook::Symbol,
    pre_disconnect_filled: i64,
    expected_total: i64,
    current_positions: &[Position],
    side: BrokerSide,
) -> Result<OrderResult, BrokerError> {
    info!(
        "AUDIT: Reconciling order {} after disconnect - symbol={}, pre_disconnect_filled={}, expected_total={}",
        order_id, symbol, pre_disconnect_filled, expected_total
    );

    // Find the position for this symbol
    let current_position = current_positions.iter().find(|p| p.symbol == symbol);

    match current_position {
        Some(pos) => {
            let current_qty = pos.quantity;

            // For buy orders, positive quantity indicates long position
            // For sell orders, negative quantity indicates short position
            let signed_current_qty = match side {
                BrokerSide::Buy => current_qty,
                BrokerSide::Sell => -current_qty,
            };

            // Compare current quantity against pre-disconnect filled
            if signed_current_qty.abs() > pre_disconnect_filled {
                // Additional shares were filled during disconnect
                let reconciled_filled = signed_current_qty.abs();
                let remainder = expected_total - reconciled_filled;

                info!(
                    "AUDIT: Order {} reconciled as PARTIAL_FILL (filled={}, remainder={}) - remainder NOT resubmitted",
                    order_id, reconciled_filled, remainder
                );

                Ok(OrderResult {
                    symbol,
                    order_id,
                    filled_shares: reconciled_filled,
                    avg_fill_price: 0.0, // Would need to query execution history for accurate price
                    commission: 0.0,     // Would need to query execution history
                    status: OrderOutcome::PartialFill,
                })
            } else if signed_current_qty.abs() == pre_disconnect_filled {
                // No additional fills during disconnect
                info!(
                    "AUDIT: Order {} no additional fills detected during disconnect (filled={})",
                    order_id, pre_disconnect_filled
                );

                Ok(OrderResult {
                    symbol,
                    order_id,
                    filled_shares: pre_disconnect_filled,
                    avg_fill_price: 0.0,
                    commission: 0.0,
                    status: if pre_disconnect_filled > 0 {
                        OrderOutcome::PartialFill
                    } else {
                        OrderOutcome::Failed
                    },
                })
            } else {
                // Position decreased - unexpected, manual review required
                warn!(
                    "AUDIT: Order {} position decreased after disconnect (pre={}, current={}) - manual review required",
                    order_id,
                    pre_disconnect_filled,
                    signed_current_qty.abs()
                );

                Ok(OrderResult {
                    symbol,
                    order_id,
                    filled_shares: pre_disconnect_filled,
                    avg_fill_price: 0.0,
                    commission: 0.0,
                    status: OrderOutcome::PartialFill,
                })
            }
        }
        None => {
            // Position not found - order may have been cancelled or no position exists
            warn!(
                "AUDIT: Order {} position not found after disconnect - manual review required",
                order_id
            );

            Ok(OrderResult {
                symbol,
                order_id,
                filled_shares: pre_disconnect_filled,
                avg_fill_price: 0.0,
                commission: 0.0,
                status: if pre_disconnect_filled > 0 {
                    OrderOutcome::PartialFill
                } else {
                    OrderOutcome::Failed
                },
            })
        }
    }
}

/// Sleep for the rate-limit interval between orders.
pub fn rate_limit_delay(interval_ms: u64) {
    if interval_ms > 0 {
        thread::sleep(Duration::from_millis(interval_ms));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_order(order_type: BrokerOrderType) -> BrokerOrder {
        BrokerOrder {
            symbol: nanobook::Symbol::try_new("AAPL").unwrap(),
            side: BrokerSide::Sell,
            quantity: 10,
            order_type,
            client_order_id: None,
        }
    }

    #[test]
    fn test_stop_order_encodes_stop_price() {
        let order = test_order(BrokerOrderType::Stop(nanobook::Price(18_250)));

        let ib_order = encode_ibkr_order(&order, None).unwrap();

        assert_eq!(ib_order.order_type, "STP");
        assert!(matches!(ib_order.action, IbAction::Sell));
        assert_eq!(ib_order.total_quantity, 10.0);
        assert_eq!(ib_order.aux_price, Some(182.50));
        assert_eq!(ib_order.limit_price, None);
    }

    fn notice_error(code: i32) -> ibapi::Error {
        ibapi::Error::Notice(ibapi::Notice {
            request_id: Some(1),
            code,
            message: "test".into(),
            error_time: None,
            advanced_order_reject_json: String::new(),
        })
    }

    #[test]
    fn classify_stream_error_separates_rejection_from_unknown() {
        // 201: order rejected.
        assert_eq!(
            classify_stream_error(&notice_error(201)),
            StreamEnd::Rejected
        );
        // Connectivity lost and a plain I/O-style error leave the order state unknown.
        assert_eq!(classify_stream_error(&notice_error(1100)), StreamEnd::Lost);
        assert_eq!(
            classify_stream_error(&ibapi::Error::ConnectionReset),
            StreamEnd::Lost
        );
    }

    #[test]
    fn unfinished_order_is_an_error_even_with_zero_fill() {
        assert!(unfinished_order_error(7, 0.0, true).is_none());
        match unfinished_order_error(7, 0.0, false) {
            Some(BrokerError::ConnectionLost {
                order_id,
                filled_quantity,
            }) => {
                assert_eq!(order_id, 7);
                assert_eq!(filled_quantity, 0);
            }
            other => panic!("expected ConnectionLost, got {other:?}"),
        }
        match unfinished_order_error(7, 40.0, false) {
            Some(BrokerError::ConnectionLost {
                filled_quantity, ..
            }) => assert_eq!(filled_quantity, 40),
            other => panic!("expected ConnectionLost, got {other:?}"),
        }
    }

    #[test]
    fn fill_price_is_never_zero_for_a_fill() {
        assert_eq!(reported_fill_price(Some(101.5)), Some(101.5));
        assert_eq!(reported_fill_price(Some(0.0)), None);
        assert_eq!(reported_fill_price(Some(f64::NAN)), None);
        assert_eq!(reported_fill_price(None), None);
        // A known price survives; an unknown price with shares filled is None.
        assert_eq!(fill_price(10.0, Some(101.5)), Some(101.5));
        assert_eq!(fill_price(10.0, None), None);
        // No fill: price 0 is correct.
        assert_eq!(fill_price(0.0, None), Some(0.0));
    }

    #[test]
    fn classify_cancel_text_decides_fill_and_failure() {
        assert_eq!(
            classify_cancel_text("Order already filled"),
            CancelText::AlreadyFilled
        );
        assert_eq!(
            classify_cancel_text("ORDER COMPLETED"),
            CancelText::AlreadyFilled
        );
        assert_eq!(
            classify_cancel_text("No such order"),
            CancelText::NoSuchOrder
        );
        assert_eq!(
            classify_cancel_text("Cannot cancel: exchange closed"),
            CancelText::CannotCancel
        );
        assert_eq!(classify_cancel_text("Order Canceled"), CancelText::Other);
    }

    #[test]
    fn cancel_failed_never_reports_unknown_order_as_cancelled() {
        // A fill phrase is a fill.
        assert_eq!(
            cancel_failed(5, "order already filled").unwrap(),
            CancelOutcome::Filled
        );
        // "No such order" is a failed cancel: the order state is unknown.
        assert!(matches!(
            cancel_failed(5, "No such order"),
            Err(BrokerError::Order(_))
        ));
        assert!(matches!(
            cancel_failed(5, "cannot cancel"),
            Err(BrokerError::CancelReject { order_id: 5, .. })
        ));
        assert!(matches!(
            cancel_failed(5, "socket closed"),
            Err(BrokerError::Order(_))
        ));
    }

    #[test]
    fn classify_notice_connection_codes() {
        assert_eq!(classify_notice(1100), NoticeAction::ConnectionLost);
        assert_eq!(classify_notice(1101), NoticeAction::ConnectionLost);
        // 1102: connectivity restored, data maintained. The order is still live.
        assert_eq!(classify_notice(1102), NoticeAction::Ignore);
    }

    #[test]
    fn classify_notice_ranges() {
        assert_eq!(classify_notice(-1), NoticeAction::Warn);
        assert_eq!(classify_notice(2104), NoticeAction::Warn);
        assert_eq!(classify_notice(2000), NoticeAction::Warn);
        assert_eq!(classify_notice(0), NoticeAction::Ignore);
        assert_eq!(classify_notice(399), NoticeAction::Ignore);
        assert_eq!(classify_notice(1999), NoticeAction::Ignore);
    }

    #[test]
    fn test_stop_limit_order_encodes_stop_and_limit_prices() {
        let order = test_order(BrokerOrderType::StopLimit {
            stop: nanobook::Price(18_250),
            limit: nanobook::Price(18_100),
        });

        let ib_order = encode_ibkr_order(&order, None).unwrap();

        assert_eq!(ib_order.order_type, "STP LMT");
        assert!(matches!(ib_order.action, IbAction::Sell));
        assert_eq!(ib_order.total_quantity, 10.0);
        assert_eq!(ib_order.aux_price, Some(182.50));
        assert_eq!(ib_order.limit_price, Some(181.00));
    }

    #[test]
    fn test_dedup_cache_detects_duplicates() {
        let cache = CallbackDedupCache::default();

        let order_id = 12345;
        let status = "Filled";
        let filled_qty: f64 = 100.0;

        // First callback should not be a duplicate
        {
            let key = OrderCallbackKey {
                order_id,
                status: status.to_string(),
                filled_quantity: filled_qty.round() as i64,
            };
            let mut cache_guard = cache.lock().unwrap();
            assert!(!cache_guard.contains_key(&key));
            cache_guard.insert(key, Instant::now());
        }

        // Second identical callback should be detected as duplicate
        {
            let key = OrderCallbackKey {
                order_id,
                status: status.to_string(),
                filled_quantity: filled_qty.round() as i64,
            };
            let cache_guard = cache.lock().unwrap();
            assert!(cache_guard.contains_key(&key));
        }

        // Different filled quantity should not be a duplicate
        {
            let key = OrderCallbackKey {
                order_id,
                status: status.to_string(),
                filled_quantity: (filled_qty + 50.0).round() as i64,
            };
            let cache_guard = cache.lock().unwrap();
            assert!(!cache_guard.contains_key(&key));
        }

        // Different status should not be a duplicate
        {
            let key = OrderCallbackKey {
                order_id,
                status: "Submitted".to_string(),
                filled_quantity: filled_qty.round() as i64,
            };
            let cache_guard = cache.lock().unwrap();
            assert!(!cache_guard.contains_key(&key));
        }
    }

    #[test]
    fn test_reconcile_filled_order_with_fill_reason() {
        // Test reconciliation when rejection reason indicates fill
        let order_id = 12345;
        let reason = "Order already filled - cannot cancel";

        let result = reconcile_filled_order(order_id, reason);
        assert!(result.is_ok());
        assert!(result.unwrap(), "Order should be reconciled as filled");
    }

    #[test]
    fn test_reconcile_filled_order_with_completed_reason() {
        // Test reconciliation when rejection reason indicates completion
        let order_id = 12345;
        let reason = "Order completed - cancel rejected";

        let result = reconcile_filled_order(order_id, reason);
        assert!(result.is_ok());
        assert!(result.unwrap(), "Order should be reconciled as filled");
    }

    #[test]
    fn test_reconcile_filled_order_with_uncertain_reason() {
        // Test reconciliation when rejection reason is ambiguous
        let order_id = 12345;
        let reason = "Cannot cancel order at this time";

        let result = reconcile_filled_order(order_id, reason);
        assert!(result.is_ok());
        assert!(!result.unwrap(), "Order state should be uncertain");
    }

    #[test]
    fn test_reconcile_filled_order_case_insensitive() {
        // Test that reconciliation is case-insensitive
        let order_id = 12345;
        let reason = "ORDER ALREADY FILLED - CANCEL REJECTED";

        let result = reconcile_filled_order(order_id, reason);
        assert!(result.is_ok());
        assert!(
            result.unwrap(),
            "Order should be reconciled as filled (case-insensitive)"
        );
    }

    #[test]
    fn test_reconcile_partial_fill_with_additional_fill() {
        // Test reconciliation when additional shares were filled during disconnect
        let order_id = 12345;
        let symbol = nanobook::Symbol::try_new("AAPL").unwrap();
        let pre_disconnect_filled = 50;
        let expected_total = 100;
        let side = BrokerSide::Buy;

        // Simulate position with 75 shares (25 additional filled during disconnect)
        let positions = vec![Position {
            symbol,
            quantity: 75,
            avg_cost_cents: 15000,
            market_value_cents: 1125000,
            unrealized_pnl_cents: 0,
        }];

        let result = reconcile_partial_fill(
            order_id,
            symbol,
            pre_disconnect_filled,
            expected_total,
            &positions,
            side,
        );

        assert!(result.is_ok());
        let order_result = result.unwrap();
        assert_eq!(order_result.order_id, order_id);
        assert_eq!(
            order_result.filled_shares, 75,
            "Should detect additional 25 shares filled"
        );
        assert_eq!(order_result.status, OrderOutcome::PartialFill);
    }

    #[test]
    fn test_reconcile_partial_fill_no_additional_fill() {
        // Test reconciliation when no additional shares were filled during disconnect
        let order_id = 12345;
        let symbol = nanobook::Symbol::try_new("AAPL").unwrap();
        let pre_disconnect_filled = 50;
        let expected_total = 100;
        let side = BrokerSide::Buy;

        // Position unchanged (no additional fills)
        let positions = vec![Position {
            symbol,
            quantity: 50,
            avg_cost_cents: 15000,
            market_value_cents: 750000,
            unrealized_pnl_cents: 0,
        }];

        let result = reconcile_partial_fill(
            order_id,
            symbol,
            pre_disconnect_filled,
            expected_total,
            &positions,
            side,
        );

        assert!(result.is_ok());
        let order_result = result.unwrap();
        assert_eq!(
            order_result.filled_shares, 50,
            "Should report pre-disconnect fill quantity"
        );
        assert_eq!(order_result.status, OrderOutcome::PartialFill);
    }

    #[test]
    fn test_reconcile_partial_fill_position_not_found() {
        // Test reconciliation when position is not found after disconnect
        let order_id = 12345;
        let symbol = nanobook::Symbol::try_new("AAPL").unwrap();
        let pre_disconnect_filled = 50;
        let expected_total = 100;
        let side = BrokerSide::Buy;

        // No positions found
        let positions: Vec<Position> = vec![];

        let result = reconcile_partial_fill(
            order_id,
            symbol,
            pre_disconnect_filled,
            expected_total,
            &positions,
            side,
        );

        assert!(result.is_ok());
        let order_result = result.unwrap();
        assert_eq!(
            order_result.filled_shares, 50,
            "Should report pre-disconnect fill quantity"
        );
        assert_eq!(order_result.status, OrderOutcome::PartialFill);
    }

    #[test]
    fn test_reconcile_partial_fill_sell_order() {
        // Test reconciliation for sell orders (negative positions)
        let order_id = 12345;
        let symbol = nanobook::Symbol::try_new("AAPL").unwrap();
        let pre_disconnect_filled = 50;
        let expected_total = 100;
        let side = BrokerSide::Sell;

        // Sell order: negative position indicates short
        let positions = vec![Position {
            symbol,
            quantity: -75, // 75 shares sold (25 additional during disconnect)
            avg_cost_cents: 15000,
            market_value_cents: -1125000,
            unrealized_pnl_cents: 0,
        }];

        let result = reconcile_partial_fill(
            order_id,
            symbol,
            pre_disconnect_filled,
            expected_total,
            &positions,
            side,
        );

        assert!(result.is_ok());
        let order_result = result.unwrap();
        assert_eq!(
            order_result.filled_shares, 75,
            "Should detect additional 25 shares sold"
        );
        assert_eq!(order_result.status, OrderOutcome::PartialFill);
    }

    #[test]
    fn test_connection_lost_error_variant() {
        // Test that ConnectionLost error variant works correctly
        let error = BrokerError::ConnectionLost {
            order_id: 12345,
            filled_quantity: 50,
        };

        let error_string = error.to_string();
        assert!(error_string.contains("connection lost"));
        assert!(error_string.contains("12345"));
        assert!(error_string.contains("50"));
    }
}
