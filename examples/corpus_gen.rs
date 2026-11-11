//! Write one random corpus `input.jsonl` from a seed and a length.
//!
//! Run with: `cargo run --release --example corpus_gen -- <seed> <length> <output.jsonl>`

use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::process::exit;

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

fn usage() -> ! {
    eprintln!("Usage: corpus_gen <seed> <length> <output.jsonl>");
    exit(1);
}

fn parse_u64(label: &str, text: &str) -> u64 {
    match text.parse::<u64>() {
        Ok(n) => n,
        Err(_) => {
            eprintln!("invalid {label}: {text}");
            exit(1);
        }
    }
}

fn side(rng: &mut SplitMix64) -> &'static str {
    if rng.next_u64().is_multiple_of(2) {
        "BUY"
    } else {
        "SELL"
    }
}

fn price(rng: &mut SplitMix64) -> i64 {
    10_000 + (rng.next_u64() % 5) as i64
}

fn quantity(rng: &mut SplitMix64) -> u64 {
    match rng.next_u64() % 16 {
        0 => u64::MAX,
        1 => u64::MAX - 1,
        2 => 1_u64 << 63,
        _ => 1 + rng.next_u64() % 10_000,
    }
}

fn tif(rng: &mut SplitMix64) -> &'static str {
    match rng.next_u64() % 3 {
        0 => "GTC",
        1 => "IOC",
        _ => "FOK",
    }
}

fn owner(rng: &mut SplitMix64) -> &'static str {
    match rng.next_u64() % 3 {
        0 => "null",
        1 => "1",
        _ => "2",
    }
}

fn stp(rng: &mut SplitMix64) -> &'static str {
    match rng.next_u64() % 4 {
        0 => "Off",
        1 => "CancelNewest",
        2 => "CancelOldest",
        _ => "DecrementAndCancel",
    }
}

fn write_limit(rng: &mut SplitMix64, out: &mut impl Write) {
    writeln!(
        out,
        r#"{{"type":"SubmitLimit","side":"{}","price":{},"quantity":{},"time_in_force":"{}","owner":{},"stp_policy":"{}"}}"#,
        side(rng),
        price(rng),
        quantity(rng),
        tif(rng),
        owner(rng),
        stp(rng),
    )
    .unwrap_or_else(|err| {
        eprintln!("write: {err}");
        exit(1);
    });
}

fn write_market(rng: &mut SplitMix64, out: &mut impl Write) {
    writeln!(
        out,
        r#"{{"type":"SubmitMarket","side":"{}","quantity":{},"owner":{},"stp_policy":"{}"}}"#,
        side(rng),
        quantity(rng),
        owner(rng),
        stp(rng),
    )
    .unwrap_or_else(|err| {
        eprintln!("write: {err}");
        exit(1);
    });
}

fn write_cancel(rng: &mut SplitMix64, submitted: u64, out: &mut impl Write) {
    let order_id = if submitted == 0 {
        1
    } else {
        1 + rng.next_u64() % submitted
    };
    writeln!(out, r#"{{"type":"Cancel","order_id":{order_id}}}"#).unwrap_or_else(|err| {
        eprintln!("write: {err}");
        exit(1);
    });
}

fn main() {
    let mut args = env::args();
    let _program = args.next();
    let Some(seed_text) = args.next() else {
        usage()
    };
    let Some(length_text) = args.next() else {
        usage()
    };
    let Some(path) = args.next() else { usage() };
    if args.next().is_some() {
        usage();
    }
    let seed = parse_u64("seed", &seed_text);
    let length = parse_u64("length", &length_text);
    let file = File::create(&path).unwrap_or_else(|err| {
        eprintln!("create {path}: {err}");
        exit(1);
    });
    let mut out = BufWriter::new(file);
    let mut rng = SplitMix64::new(seed);
    let mut submitted = 0_u64;
    for _ in 0..length {
        match rng.next_u64() % 10 {
            0..=2 => {
                write_market(&mut rng, &mut out);
                submitted += 1;
            }
            3..=5 => write_cancel(&mut rng, submitted, &mut out),
            _ => {
                write_limit(&mut rng, &mut out);
                submitted += 1;
            }
        }
    }
    // Dropping a BufWriter discards a flush error; report it instead.
    out.flush().unwrap_or_else(|err| {
        eprintln!("write {path}: {err}");
        exit(1);
    });
}
