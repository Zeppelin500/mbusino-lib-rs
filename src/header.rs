//! Fixed data header of a long-frame telegram - counterpart of the C++
//! `decodeHeader()`.
//!
//! `decodeHeader()` maps to `Option<Header>`:
//!
//! * `None`                     - C++ returned `false` *before* writing
//!                                anything (too short, wrong start byte or
//!                                two different length fields) -> `{}`
//! * `Some(h)` with `id == None` - C++ returned `false` *after* writing the
//!                                basic fields (CI is not 0x72/0x76)
//! * `Some(h)` with `id != None` - C++ returned `true` (full header)

use crate::json::Json;

/// Status byte breakdown (nested `"status_details"` object).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusDetails {
    pub battery_low: bool,
    pub permanent_error: bool,
    pub temporary_error: bool,
    /// `Some` except for the reserved value 0x03 (C++ writes no key there).
    pub app_status: Option<&'static str>,
}

impl StatusDetails {
    pub fn to_json(&self) -> Json {
        let mut obj = Json::object();
        obj.set("battery_low", self.battery_low);
        obj.set("permanent_error", self.permanent_error);
        obj.set("temporary_error", self.temporary_error);
        if let Some(v) = self.app_status {
            obj.set("app_status", v);
        }
        obj
    }
}

/// Decoded fixed data header (all fields optional, see module docs).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Header {
    pub len: Option<u8>,
    pub c_field: Option<u8>,
    pub address: Option<u8>,
    pub ci_field: Option<u8>,
    pub id: Option<String>,
    pub manufacturer: Option<String>,
    pub version: Option<u8>,
    pub medium_code: Option<u8>,
    pub medium: Option<String>,
    pub access_counter: Option<u8>,
    pub status: Option<u8>,
    pub status_details: Option<StatusDetails>,
    pub signature: Option<u16>,
}

impl Header {
    /// `true` when the full fixed data header was decoded (C++ `return true`).
    pub fn is_ok(&self) -> bool {
        self.id.is_some()
    }

    /// Serialise to the ArduinoJson-shaped [`Json`] value.
    pub fn to_json(&self) -> Json {
        let mut obj = Json::object();
        if let Some(v) = self.len {
            obj.set("len", v);
        }
        if let Some(v) = self.c_field {
            obj.set("c_field", v);
        }
        if let Some(v) = self.address {
            obj.set("address", v);
        }
        if let Some(v) = self.ci_field {
            obj.set("ci_field", v);
        }
        if let Some(ref v) = self.id {
            obj.set("id", v.clone());
        }
        if let Some(ref v) = self.manufacturer {
            obj.set("manufacturer", v.clone());
        }
        if let Some(v) = self.version {
            obj.set("version", v);
        }
        if let Some(v) = self.medium_code {
            obj.set("medium_code", v);
        }
        if let Some(ref v) = self.medium {
            obj.set("medium", v.clone());
        }
        if let Some(v) = self.access_counter {
            obj.set("access_counter", v);
        }
        if let Some(v) = self.status {
            obj.set("status", v);
        }
        if let Some(ref v) = self.status_details {
            obj.set("status_details", v.to_json());
        }
        if let Some(v) = self.signature {
            obj.set("signature", v);
        }
        obj
    }

    /// Convenience: JSON string of this header.
    pub fn to_json_string(&self) -> String {
        self.to_json().to_json_string()
    }
}
