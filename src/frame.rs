//! Frame checks and the complete telegram result.
//!
//! `decodeTelegram()` is a *new* addition (not present in the C++ original):
//! it splits a complete telegram apart like `test_decode.cpp` does and reports
//! frame-level checks. `Frame` is the `"frame"` object of the result.

use crate::header::Header;
use crate::json::Json;
use crate::record::Record;

/// Frame sanity checks (the `"frame"` object).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Frame {
    pub byte_count: usize,
    pub packet_size: Option<usize>,
    pub slave_address: Option<u8>,
    pub c_field: Option<u8>,
    pub ci_field: Option<u8>,
    pub start_ok: bool,
    pub length_ok: bool,
    pub fits: bool,
    pub stop_ok: bool,
    pub checksum: Option<u32>,
    pub checksum_received: Option<u32>,
    pub checksum_ok: bool,
    pub quartet_checksum_ok: bool,
    pub ok: bool,
    pub error_message: String,
}

impl Frame {
    pub fn to_json(&self) -> Json {
        fn opt(v: Option<u64>) -> Json {
            match v {
                Some(n) => Json::UInt(n),
                None => Json::Null,
            }
        }
        let mut obj = Json::object();
        obj.set("byte_count", self.byte_count as u64);
        obj.set("packet_size", opt(self.packet_size.map(|v| v as u64)));
        obj.set("slave_address", opt(self.slave_address.map(|v| v as u64)));
        obj.set("c_field", opt(self.c_field.map(|v| v as u64)));
        obj.set("ci_field", opt(self.ci_field.map(|v| v as u64)));
        obj.set("start_ok", self.start_ok);
        obj.set("length_ok", self.length_ok);
        obj.set("fits", self.fits);
        obj.set("stop_ok", self.stop_ok);
        obj.set("checksum", opt(self.checksum.map(|v| v as u64)));
        obj.set(
            "checksum_received",
            opt(self.checksum_received.map(|v| v as u64)),
        );
        obj.set("checksum_ok", self.checksum_ok);
        obj.set("quartet_checksum_ok", self.quartet_checksum_ok);
        obj.set("ok", self.ok);
        obj.set("error_message", self.error_message.clone());
        obj
    }
}

/// Complete telegram decode result (`decode_telegram()`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Telegram {
    pub frame: Frame,
    pub header: Option<Header>,
    pub records: Vec<Record>,
    /// `MBUS_ERROR` code of the decoding step.
    pub error: u8,
    /// English message naming the first problem found.
    pub error_message: String,
}

impl Telegram {
    pub fn to_json(&self) -> Json {
        let mut obj = Json::object();
        obj.set("frame", self.frame.to_json());
        match &self.header {
            Some(h) => obj.set("header", h.to_json()),
            None => obj.set("header", Json::object()),
        }
        let mut arr = Json::array();
        for record in &self.records {
            arr.push(record.to_json());
        }
        obj.set("records", arr);
        obj.set("error", self.error as u64);
        obj.set("error_message", self.error_message.clone());
        obj
    }

    pub fn to_json_string(&self) -> String {
        self.to_json().to_json_string()
    }
}
