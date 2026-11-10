//! Error codes - 1:1 port of `enum MBUS_ERROR` from `MBusinoLib.h`.
//!
//! Only the three codes the decoder can actually set are kept. The C++
//! enum still numbers the removed encoder leftovers `UNSUPPORTED_CODING`
//! (2), `UNSUPPORTED_RANGE` (3) and `NEGATIVE_VALUE` (5); the gaps in the
//! numbering are deliberate so the surviving codes keep their C++ values
//! (`NO_ERROR = 0`, `BUFFER_OVERFLOW = 1`, `UNSUPPORTED_VIF = 4`) and the
//! decode output, error messages and golden files stay identical.

/// Error codes, matching the C++ `enum MBUS_ERROR` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MbusError {
    NoError = 0,
    BufferOverflow = 1,
    UnsupportedVif = 4,
}

impl MbusError {
    /// Convert a raw code to the enum; unknown codes map to `NoError`
    /// (the decoder never produces other values).
    pub fn from_code(code: u8) -> Self {
        match code {
            1 => MbusError::BufferOverflow,
            4 => MbusError::UnsupportedVif,
            _ => MbusError::NoError,
        }
    }

    /// Numeric value (for comparison with the golden references).
    pub fn code(self) -> u8 {
        self as u8
    }

    /// Human readable English message (`get_error_message()`).
    pub fn message(self) -> &'static str {
        match self {
            MbusError::NoError => "No error.",
            MbusError::BufferOverflow => {
                "Buffer too small or telegram incomplete: the declared length does not \
                 fit into the given data, or a record would run past the end of the buffer."
            }
            MbusError::UnsupportedVif => {
                "Unknown or unsupported VIF - the record could not be matched to any \
                 definition in vif_defs."
            }
        }
    }
}

/// Same message lookup, but for a raw code (used where a code could come
/// from outside the enum).
pub fn error_message(code: u8) -> String {
    match code {
        0 => MbusError::NoError.message().to_string(),
        1 => MbusError::BufferOverflow.message().to_string(),
        4 => MbusError::UnsupportedVif.message().to_string(),
        other => format!("Unknown error code {}.", other),
    }
}
