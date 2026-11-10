//! Minimal, dependency-free JSON writer.
//!
//! The crate intentionally has **no runtime dependencies** (see the task:
//! "Keine Runtime-Dependencies"), so instead of pulling in `serde` we emit
//! JSON ourselves. Only what the ArduinoJson output of the original library
//! needs is supported: objects, arrays, strings, booleans, integers and
//! floats.
//!
//! Key order is preserved, and - like ArduinoJson with the default
//! configuration - object members with a missing value are simply left out
//! (the caller only adds keys that are actually present).

/// A tiny JSON value builder.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    Str(String),
    Array(Vec<Json>),
    /// Ordered object (insertion order is preserved on output).
    Object(Vec<(String, Json)>),
}

impl Json {
    /// New empty object.
    pub fn object() -> Self {
        Json::Object(Vec::new())
    }

    /// New empty array.
    pub fn array() -> Self {
        Json::Array(Vec::new())
    }

    /// Push a member onto an object (no-op on other variants).
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<Json>) {
        if let Json::Object(members) = self {
            members.push((key.into(), value.into()));
        }
    }

    /// Push an element onto an array (no-op on other variants).
    pub fn push(&mut self, value: impl Into<Json>) {
        if let Json::Array(items) = self {
            items.push(value.into());
        }
    }

    /// Serialise this value to a compact JSON string.
    pub fn to_json_string(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Int(v) => out.push_str(&v.to_string()),
            Json::UInt(v) => out.push_str(&v.to_string()),
            Json::Float(v) => write_float(*v, out),
            Json::Str(s) => write_string(s, out),
            Json::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Object(members) => {
                out.push('{');
                for (i, (key, value)) in members.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(key, out);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}

impl From<bool> for Json {
    fn from(v: bool) -> Self {
        Json::Bool(v)
    }
}
impl From<i64> for Json {
    fn from(v: i64) -> Self {
        Json::Int(v)
    }
}
impl From<i32> for Json {
    fn from(v: i32) -> Self {
        Json::Int(v as i64)
    }
}
impl From<u64> for Json {
    fn from(v: u64) -> Self {
        Json::UInt(v)
    }
}
impl From<u32> for Json {
    fn from(v: u32) -> Self {
        Json::UInt(v as u64)
    }
}
impl From<u8> for Json {
    fn from(v: u8) -> Self {
        Json::UInt(v as u64)
    }
}
impl From<u16> for Json {
    fn from(v: u16) -> Self {
        Json::UInt(v as u64)
    }
}
impl From<usize> for Json {
    fn from(v: usize) -> Self {
        Json::UInt(v as u64)
    }
}
impl From<f64> for Json {
    fn from(v: f64) -> Self {
        Json::Float(v)
    }
}
impl From<String> for Json {
    fn from(v: String) -> Self {
        Json::Str(v)
    }
}
impl From<&str> for Json {
    fn from(v: &str) -> Self {
        Json::Str(v.to_string())
    }
}
/// Format an `f64` like Rust's `Display` (shortest round-tripping decimal,
/// never scientific notation - which keeps large meter values readable, e.g.
/// `202310171000`).
fn write_float(v: f64, out: &mut String) {
    if v.is_finite() {
        out.push_str(&format!("{}", v));
    } else {
        // JSON has no NaN/Infinity literals.
        out.push_str("null");
    }
}

/// Escape a JSON string (UTF-8 bytes are kept as-is, like ArduinoJson).
fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
