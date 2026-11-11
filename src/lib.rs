//! # mbusino-lib-rs
//!
//! A dependency-free Rust port of the Arduino library **MBusinoLib**, which
//! decodes M-Bus (Meterbus, EN 13757) `RSP_UD` telegrams - the answer a slave
//! sends to the master.
//!
//! The port follows the C++ original (`MBusinoLib`, version 0.11.0, GPL-3.0)
//! function by function. The blueprint is the verified 1:1 Python port
//! `MBusinoLibPy`, whose golden references were produced by the *unmodified*
//! C++ library; this crate reproduces the same output (see `tests/`).
//!
//! All numeric operations run in the same order and with the same types as
//! the C++ code (`int8_t` / `int16_t` / `int32_t` wrap-around, `int8_t`
//! truncation of the scalar, 32-bit `float` for "real" records), so the
//! values are bit-identical. Only ArduinoJson's *output* rounds, therefore
//! `value_scaled` is compared with a tolerance (relative 1e-6 / absolute 1e-6).
//!
//! ## Quick start
//!
//! ```
//! use mbusino_lib_rs::Decoder;
//!
//! // 68 C1 C1 68 08 00 72 ... (EFE heat meter)
//! let hex = "68 0F 0F 68 08 00 72 09 34 75 73 C5 14 00 0D 43 00 00 00 C8 16";
//! let telegram = mbusino_lib_rs::parse_hex(hex);
//! let mut decoder = Decoder::new();
//! let result = decoder.decode_telegram(&telegram);
//! assert!(result.frame.ok);
//! ```

#![forbid(unsafe_code)]

pub mod decoder;
pub mod error;
pub mod frame;
pub mod header;
pub mod json;
pub mod record;
pub mod tables;

pub use crate::decoder::{
    code_name, code_units, device_class, find_definition, get_vif, parse_hex, state_class,
    volume_class_from_medium, Decoder, VALUE_ABS_TOL, VALUE_REL_TOL,
};
pub use crate::error::{error_message, MbusError};
pub use crate::frame::{Frame, Telegram};
pub use crate::header::{Header, StatusDetails};
pub use crate::json::Json;
pub use crate::record::Record;

/// Version of the crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
