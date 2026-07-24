//! Replay golden-corpus JSONL events through the Rust Exchange and emit trades.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::exit;

use nanobook::{Exchange, OrderId, OrderOwner, Price, Side, StpPolicy, TimeInForce, Trade};

#[derive(Debug, Clone, PartialEq, Eq)]
enum CorpusEvent {
    SubmitLimit {
        side: Side,
        price: i64,
        quantity: u64,
        time_in_force: TimeInForce,
        owner: Option<u32>,
        stp_policy: StpPolicy,
    },
    SubmitMarket {
        side: Side,
        quantity: u64,
        owner: Option<u32>,
        stp_policy: StpPolicy,
    },
    Cancel {
        order_id: u64,
    },
}

#[cfg(test)]
impl CorpusEvent {
    fn side(&self) -> &'static str {
        match self {
            CorpusEvent::SubmitLimit { side, .. } | CorpusEvent::SubmitMarket { side, .. } => {
                match side {
                    Side::Buy => "BUY",
                    Side::Sell => "SELL",
                }
            }
            CorpusEvent::Cancel { .. } => "",
        }
    }

    fn owner(&self) -> Option<u32> {
        match self {
            CorpusEvent::SubmitLimit { owner, .. } | CorpusEvent::SubmitMarket { owner, .. } => {
                *owner
            }
            CorpusEvent::Cancel { .. } => None,
        }
    }

    fn stp_policy(&self) -> &'static str {
        match self {
            CorpusEvent::SubmitLimit { stp_policy, .. }
            | CorpusEvent::SubmitMarket { stp_policy, .. } => match *stp_policy {
                StpPolicy::Off => "Off",
                StpPolicy::CancelNewest => "CancelNewest",
                StpPolicy::CancelOldest => "CancelOldest",
                StpPolicy::DecrementAndCancel => "DecrementAndCancel",
            },
            CorpusEvent::Cancel { .. } => "",
        }
    }
}

fn stp_policy_from_corpus_str(s: &str) -> Option<StpPolicy> {
    match s {
        "Off" => Some(StpPolicy::Off),
        "CancelNewest" => Some(StpPolicy::CancelNewest),
        "CancelOldest" => Some(StpPolicy::CancelOldest),
        "DecrementAndCancel" => Some(StpPolicy::DecrementAndCancel),
        _ => None,
    }
}

fn parse_side(s: &str) -> Option<Side> {
    match s {
        "BUY" => Some(Side::Buy),
        "SELL" => Some(Side::Sell),
        _ => None,
    }
}

fn parse_tif(s: &str) -> Option<TimeInForce> {
    match s {
        "GTC" => Some(TimeInForce::GTC),
        "IOC" => Some(TimeInForce::IOC),
        "FOK" => Some(TimeInForce::FOK),
        _ => None,
    }
}

fn json_string_field<'a>(obj: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(|v| v.as_str())
}

fn json_i64_field(obj: &serde_json::Value, key: &str) -> Option<i64> {
    let v = obj.get(key)?;
    if let Some(n) = v.as_i64() {
        return Some(n);
    }
    v.as_str().and_then(|s| s.parse().ok())
}

fn json_u64_field(obj: &serde_json::Value, key: &str) -> Option<u64> {
    obj.get(key)
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|n| n as u64)))
}

fn json_owner_field(obj: &serde_json::Value) -> Option<Option<u32>> {
    match obj.get("owner") {
        None => Some(None),
        Some(v) if v.is_null() => Some(None),
        Some(v) => v
            .as_u64()
            .or_else(|| v.as_i64().map(|n| n as u64))
            .map(|n| Some(n as u32)),
    }
}

fn json_stp_field(obj: &serde_json::Value) -> Option<StpPolicy> {
    match obj.get("stp_policy") {
        None => Some(StpPolicy::Off),
        Some(v) => v.as_str().and_then(stp_policy_from_corpus_str),
    }
}

fn parse_event_line(line: &str) -> Result<CorpusEvent, String> {
    let line = line.trim();
    if line.is_empty() {
        return Err("empty line".into());
    }

    let obj: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("invalid json: {e}"))?;
    let event_type = json_string_field(&obj, "type").ok_or("missing type")?;

    match event_type {
        "SubmitLimit" => {
            let side = json_string_field(&obj, "side")
                .and_then(parse_side)
                .ok_or("invalid side")?;
            let price = json_i64_field(&obj, "price").ok_or("invalid price")?;
            let quantity = json_u64_field(&obj, "quantity").ok_or("invalid quantity")?;
            let time_in_force = json_string_field(&obj, "time_in_force")
                .and_then(parse_tif)
                .ok_or("invalid time_in_force")?;
            let owner = json_owner_field(&obj).ok_or("invalid owner")?;
            let stp_policy = json_stp_field(&obj).ok_or("invalid stp_policy")?;
            Ok(CorpusEvent::SubmitLimit {
                side,
                price,
                quantity,
                time_in_force,
                owner,
                stp_policy,
            })
        }
        "SubmitMarket" => {
            let side = json_string_field(&obj, "side")
                .and_then(parse_side)
                .ok_or("invalid side")?;
            let quantity = json_u64_field(&obj, "quantity").ok_or("invalid quantity")?;
            let owner = json_owner_field(&obj).ok_or("invalid owner")?;
            let stp_policy = json_stp_field(&obj).ok_or("invalid stp_policy")?;
            Ok(CorpusEvent::SubmitMarket {
                side,
                quantity,
                owner,
                stp_policy,
            })
        }
        "Cancel" => {
            let order_id = json_u64_field(&obj, "order_id").ok_or("invalid order_id")?;
            Ok(CorpusEvent::Cancel { order_id })
        }
        _ => Err(format!("unknown event type: {event_type}")),
    }
}

fn replay_events(events: &[CorpusEvent]) -> Vec<Trade> {
    let mut exchange = Exchange::new();

    for event in events {
        match event {
            CorpusEvent::SubmitLimit {
                side,
                price,
                quantity,
                time_in_force,
                owner,
                stp_policy,
            } => {
                exchange.set_stp_policy(*stp_policy);
                if let Some(owner_id) = *owner {
                    exchange.submit_limit_with_owner(
                        *side,
                        Price(*price),
                        *quantity,
                        *time_in_force,
                        OrderOwner(owner_id),
                    );
                } else {
                    exchange.submit_limit(*side, Price(*price), *quantity, *time_in_force);
                }
            }
            CorpusEvent::SubmitMarket {
                side,
                quantity,
                owner,
                stp_policy,
            } => {
                exchange.set_stp_policy(*stp_policy);
                let price = match side {
                    Side::Buy => Price::MAX,
                    Side::Sell => Price::MIN,
                };
                if let Some(owner_id) = *owner {
                    exchange.submit_limit_with_owner(
                        *side,
                        price,
                        *quantity,
                        TimeInForce::IOC,
                        OrderOwner(owner_id),
                    );
                } else {
                    exchange.submit_market(*side, *quantity);
                }
            }
            CorpusEvent::Cancel { order_id } => {
                exchange.cancel(OrderId(*order_id));
            }
        }
    }

    exchange.trades().to_vec()
}

fn replay_file(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for (idx, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("read line {}: {e}", idx + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        events.push(parse_event_line(&line)?);
    }

    let trades = replay_events(&events);
    Ok(trades
        .iter()
        .map(trade_to_json_line)
        .collect::<Vec<_>>()
        .join("\n"))
}

fn trade_to_json_line(trade: &Trade) -> String {
    format!(
        r#"{{"id":{},"price":{},"quantity":{},"aggressor_order_id":{},"passive_order_id":{},"aggressor_side":"{}","timestamp":{}}}"#,
        trade.id.0,
        trade.price.0,
        trade.quantity,
        trade.aggressor_order_id.0,
        trade.passive_order_id.0,
        trade.aggressor_side,
        trade.timestamp,
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: corpus-replay <input.jsonl> <output.jsonl>");
        exit(1);
    }

    let input = Path::new(&args[1]);
    let output = Path::new(&args[2]);

    let jsonl = match replay_file(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            exit(1);
        }
    };

    let mut out = File::create(output).unwrap_or_else(|e| {
        eprintln!("create {}: {e}", output.display());
        exit(1);
    });
    if !jsonl.is_empty() {
        writeln!(out, "{jsonl}").unwrap_or_else(|e| {
            eprintln!("write {}: {e}", output.display());
            exit(1);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_submit_limit_with_owner_and_stp_policy() {
        let event = parse_event_line(
            r#"{"type":"SubmitLimit","side":"BUY","price":10000,"quantity":50,"time_in_force":"GTC","owner":1,"stp_policy":"CancelNewest"}"#,
        )
        .expect("event parses");

        assert_eq!(event.side(), "BUY");
        assert_eq!(event.owner(), Some(1));
        assert_eq!(event.stp_policy(), "CancelNewest");
    }

    #[test]
    fn replays_case_01_to_expected_trade_jsonl() {
        let trades = replay_file(Path::new(
            "oracle-ocaml/test/corpus/01-simple-cross/input.jsonl",
        ))
        .expect("case 01 replays");

        assert_eq!(
            trades,
            r#"{"id":1,"price":10100,"quantity":50,"aggressor_order_id":2,"passive_order_id":1,"aggressor_side":"BUY","timestamp":3}"#
        );
    }
}
