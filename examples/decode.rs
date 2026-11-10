//! Decode a hex telegram and print the JSON result to stdout.
//!
//! Usage::
//!
//!     cargo run --example decode -- "68 56 56 68 ..."
//!     cargo run --example decode -- ./tests/golden/efe_heat_meter.hex
//!
//! Without an argument the built-in EFE heat-meter example is used.

use std::io::IsTerminal;
use std::io::Read;
use std::process::ExitCode;

use mbusino_lib_rs::{parse_hex, Decoder};

const DEFAULT_HEX: &str = "68 0F 0F 68 08 00 72 09 34 75 73 C5 14 00 0D 43 00 00 00 C8 16";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let input = if let Some(first) = args.first() {
        if std::path::Path::new(first).is_file() {
            match std::fs::read_to_string(first) {
                Ok(text) => text,
                Err(err) => {
                    eprintln!("error: cannot read {}: {}", first, err);
                    return ExitCode::FAILURE;
                }
            }
        } else {
            first.clone()
        }
    } else if !std::io::stdin().is_terminal() {
        let mut buf = String::new();
        let _ = std::io::stdin().read_to_string(&mut buf);
        if buf.trim().is_empty() {
            DEFAULT_HEX.to_string()
        } else {
            buf
        }
    } else {
        DEFAULT_HEX.to_string()
    };

    let data = parse_hex(&input);
    if data.is_empty() {
        eprintln!("error: no bytes parsed from input");
        return ExitCode::FAILURE;
    }

    let mut decoder = Decoder::new();
    let result = decoder.decode_telegram(&data);
    println!("{}", result.to_json_string());

    if result.frame.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
