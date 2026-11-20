//! IBKR market-data helpers.

use crate::types::{BestQuote, Quote};

/// Extract a usable NBBO quote from a broker quote snapshot.
pub(crate) fn best_quote_from_quote(quote: &Quote) -> Option<BestQuote> {
    if quote.bid_cents > 0 && quote.ask_cents > 0 {
        Some(BestQuote {
            bid_cents: quote.bid_cents,
            ask_cents: quote.ask_cents,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanobook::Symbol;
    use std::time::SystemTime;

    fn quote(bid_cents: i64, ask_cents: i64) -> Quote {
        Quote {
            symbol: Symbol::new("AAPL"),
            bid_cents,
            ask_cents,
            last_cents: 185_00,
            volume: 0,
            timestamp: SystemTime::now(),
        }
    }

    #[test]
    fn two_sided_quote_gives_best_quote() {
        assert_eq!(
            best_quote_from_quote(&quote(184_95, 185_05)),
            Some(BestQuote {
                bid_cents: 184_95,
                ask_cents: 185_05,
            })
        );
    }

    #[test]
    fn one_sided_or_empty_quote_gives_none() {
        assert_eq!(best_quote_from_quote(&quote(0, 185_05)), None);
        assert_eq!(best_quote_from_quote(&quote(184_95, 0)), None);
        assert_eq!(best_quote_from_quote(&quote(0, 0)), None);
    }
}
