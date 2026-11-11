//! Decode a raw record buffer (no telegram frame) and print the JSON result.
//!
//! Usage::
//!
//!     cargo run --example records -- "01 13 39"
//!
//! The optional second argument is the `size` parameter (default: all bytes).
//! Output: `{"error":<n>,"records":[...]}`.

use std::io::Read;
use std::process::ExitCode;

use mbusino_lib_rs::{parse_hex, Decoder};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let input = if let Some(first) = args.first() {
        if std::path::Path::new(first).is_file() {
            std::fs::read_to_string(first).unwrap_or_default()
        } else {
            first.clone()
        }
    } else {
        let mut buf = String::new();
        let _ = std::io::stdin().read_to_string(&mut buf);
        buf
    };

    let data = parse_hex(&input);
    let size = args.get(1).and_then(|s| s.parse::<usize>().ok());

    let mut decoder = Decoder::new();
    let records = decoder.decode_records(&data, size);
    let error = decoder.get_error();

    let mut arr = mbusino_lib_rs::Json::array();
    for record in &records {
        arr.push(record.to_json());
    }
    let mut obj = mbusino_lib_rs::Json::object();
    obj.set("error", error);
    obj.set("records", arr);
    println!("{}", obj.to_json_string());
    ExitCode::SUCCESS
}
