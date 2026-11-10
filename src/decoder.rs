//! Core decoder - 1:1 port of `MBusinoLib.cpp`.
//!
//! Every method states the C++ function it mirrors. All numeric operations
//! run in the same order and with the same types as the C++ code (`int8_t` /
//! `int16_t` / `int32_t` wrap-around, `int8_t` truncation of the scalar,
//! 32-bit `float` for "real" records), so the values are bit-identical; the
//! golden tests compare `value_scaled` with the same tolerance as the Python
//! port (relative 1e-6 / absolute 1e-6, see `ARDUINO_FLOAT_DECIMALS`).

use crate::error::{error_message, MbusError};
use crate::header::{Header, StatusDetails};
use crate::record::Record;
use crate::tables::{
    CODE_NAME, CODE_UNITS, DEVICE_CLASS, MEDIUM_NAMES, MEDIUM_TO_VOLUME_CLASS, STATE_CLASS,
    VIF_DEFS,
};

/// Tolerance for `value_scaled` comparisons (`ARDUINO_FLOAT_DECIMALS = 6`).
pub const VALUE_REL_TOL: f64 = 1e-6;
/// Absolute tolerance for `value_scaled` comparisons.
pub const VALUE_ABS_TOL: f64 = 1e-6;

// ---------------------------------------------------------------------------
// DIF table (C++: switch(difLeast4bit) in decodeRecords())
// index = low 4 bits of the DIF, value = (length in bytes, data coding type)
//   type: 0 = no data, 1 = integer, 2 = BCD, 3 = real,
//         4 = variable, 5 = special function
// ---------------------------------------------------------------------------
const DIF_TABLE: [(usize, u8); 16] = [
    (0, 0), // 0x00 -> No data
    (1, 1), // 0x01 -> 8 Bit Integer
    (2, 1), // 0x02 -> 16 Bit Integer
    (3, 1), // 0x03 -> 24 Bit Integer
    (4, 1), // 0x04 -> 32 Bit Integer
    (4, 3), // 0x05 -> 32 Bit Real
    (6, 1), // 0x06 -> 48 Bit Integer
    (8, 1), // 0x07 -> 64 Bit Integer
    (0, 0), // 0x08 -> Selection for Readout, not supported
    (1, 2), // 0x09 -> 2 digit BCD
    (2, 2), // 0x0A -> 4 digit BCD
    (3, 2), // 0x0B -> 6 digit BCD
    (4, 2), // 0x0C -> 8 digit BCD
    (0, 4), // 0x0D -> variable length
    (6, 2), // 0x0E -> 12 digit BCD
    (0, 5), // 0x0F -> Special Functions
];

/// Function field (C++: switch(difFunctionField)); bits 4+5 of the DIF.
const FUNCTION_FIELDS: [&str; 4] = ["", "_max", "_min", "_err"];

/// Name suffixes for the "reading user" VIFE 0xFC/0xFF.
fn phase_suffix(value: u32) -> Option<&'static str> {
    Some(match value {
        1 => "_L1",
        2 => "_L2",
        3 => "_L3",
        4 => "_N",
        5 => "_L1-L2",
        6 => "_L2-L3",
        7 => "_L3-L1",
        10 => "_abs.",
        12 => "_delta",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// C++ type semantics helpers
// ---------------------------------------------------------------------------
#[inline]
fn to_int8(value: i64) -> i8 {
    value as i8
}
#[inline]
fn to_int16(value: i64) -> i16 {
    value as i16
}
#[inline]
fn to_int32(value: i64) -> i32 {
    value as i32
}

/// Read one byte like `buffer[pos]` in C++; out of bounds is an error.
#[inline]
fn rd(data: &[u8], pos: isize) -> Result<u8, ()> {
    if pos >= 0 && (pos as usize) < data.len() {
        Ok(data[pos as usize])
    } else {
        Err(())
    }
}

/// Latin-1 decode (one byte -> one `char`), like Python's `decode("latin-1")`.
fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

// ---------------------------------------------------------------------------
// Lookup tables (C++: the four switch methods + _volumeClassFromMedium())
// ---------------------------------------------------------------------------
fn lookup(pairs: &[(u32, &'static str)], code: u32) -> Option<&'static str> {
    pairs.iter().find(|(c, _)| *c == code).map(|(_, v)| *v)
}

/// C++ `getCodeName()`.
pub fn code_name(code: u32) -> &'static str {
    lookup(CODE_NAME, code).unwrap_or("")
}

/// C++ `getCodeUnits()` (`None` corresponds to `return 0` / NULL).
pub fn code_units(code: u32) -> Option<&'static str> {
    lookup(CODE_UNITS, code)
}

/// C++ `_volumeClassFromMedium()` helper.
pub fn volume_class_from_medium(medium: u8) -> &'static str {
    MEDIUM_TO_VOLUME_CLASS
        .iter()
        .find(|(m, _)| *m == medium)
        .map(|(_, v)| *v)
        .unwrap_or("volume")
}

/// C++ `getDeviceClass()`.
pub fn device_class(code: u32, medium: u8) -> &'static str {
    // The three volume codes call the medium helper (case VOLUME_M3/FT3/GAL).
    if code == 3 || code == 106 || code == 107 {
        return volume_class_from_medium(medium);
    }
    lookup(DEVICE_CLASS, code).unwrap_or("")
}

/// C++ `getStateClass()`.
pub fn state_class(code: u32) -> &'static str {
    lookup(STATE_CLASS, code).unwrap_or("")
}

/// C++ `_findDefinition()`; `None` corresponds to the C++ `return -1`.
pub fn find_definition(vif: u32) -> Option<usize> {
    VIF_DEFS
        .iter()
        .position(|&(_code, base, size, _scalar)| base <= vif && vif < base + size)
}

/// C++ `_getVIF()`; `0xFF` means "this is not a valid VIF".
pub fn get_vif(code: u32, scalar: i32) -> i64 {
    for &(c, base, size, def_scalar) in VIF_DEFS {
        if c == code {
            let ds = def_scalar as i32;
            if ds <= scalar && scalar < ds + size as i32 {
                return base as i64 + (scalar - ds) as i64;
            }
        }
    }
    0xFF
}

// ===========================================================================
// The decoder
// ===========================================================================
/// Decoder for M-Bus RSP_UD telegrams - counterpart of the C++ `MBusinoLib`.
///
/// Holds only the error state (the internal buffer of the C++ class is
/// dropped, just like in the Python port - bytes are passed as arguments).
#[derive(Debug, Clone, Default)]
pub struct Decoder {
    error: u8,
}

impl Decoder {
    /// C++ `MBusinoLib::MBusinoLib()` - error starts at `NO_ERROR`.
    pub fn new() -> Self {
        Decoder { error: 0 }
    }

    /// C++ `getError()` - returns the error code **and clears** it.
    pub fn get_error(&mut self) -> u8 {
        let error = self.error;
        self.error = 0;
        error
    }

    /// The currently stored error code (without clearing it).
    pub fn error(&self) -> u8 {
        self.error
    }

    /// NEW (not in C++): readable English error message; clears nothing.
    pub fn get_error_message(&self) -> String {
        error_message(self.error)
    }

    /// Message for an explicit code (`get_error_message(code)`).
    pub fn error_message_for(code: u8) -> String {
        error_message(code)
    }

    /// C++ `getCodeName()`.
    pub fn get_code_name(&self, code: u32) -> &'static str {
        code_name(code)
    }

    /// C++ `getCodeUnits()`.
    pub fn get_code_units(&self, code: u32) -> Option<&'static str> {
        code_units(code)
    }

    /// C++ `getDeviceClass()`.
    pub fn get_device_class(&self, code: u32, medium: u8) -> &'static str {
        device_class(code, medium)
    }

    /// C++ `getStateClass()`.
    pub fn get_state_class(&self, code: u32) -> &'static str {
        state_class(code)
    }

    // ------------------------------------------------------------------
    // Records
    // ------------------------------------------------------------------
    /// C++ `decodeRecords()`.
    ///
    /// `size` behaves like the C++ `uint8_t size`; `None` means
    /// `buffer.len()`.
    pub fn decode_records(&mut self, buffer: &[u8], size: Option<usize>) -> Vec<Record> {
        let size = size.unwrap_or(buffer.len());
        let (records, error) = decode_records_inner(buffer, size);
        if error != 0 {
            self.error = error;
        }
        records
    }

    // ------------------------------------------------------------------
    // Header
    // ------------------------------------------------------------------
    /// C++ `decodeHeader()`.
    ///
    /// `length` is the total length that may be read (`None` = `buffer.len()`);
    /// the original expects the `packet_size` (`data[1] + 6`) here.
    /// See the `Header` type for the mapping to the C++ boolean return value.
    pub fn decode_header(&self, buffer: &[u8], length: Option<usize>) -> Option<Header> {
        let data = buffer;
        let length = length.unwrap_or(data.len());

        // 1. minimum length for a long frame with fixed data header
        if length < 19 || data.len() < 19 {
            return None;
        }
        // 2. start bytes 68 ... 68
        if data[0] != 0x68 || data[3] != 0x68 {
            return None;
        }
        // 3. both length fields must be identical
        if data[1] != data[2] {
            return None;
        }

        let mut header = Header {
            len: Some(data[1]),
            c_field: Some(data[4]),
            address: Some(data[5]),
            ci_field: Some(data[6]),
            ..Default::default()
        };

        // Only CI 0x72 (long/variable) and 0x76 (short/variable) carry the
        // fixed data header (C++: `if (buffer[6] == 0x72 || buffer[6] == 0x76)`).
        if data[6] != 0x72 && data[6] != 0x76 {
            return Some(header);
        }

        // 4. identification number, 4 byte BCD, LSB first
        header.id = Some(format!(
            "{:02X}{:02X}{:02X}{:02X}",
            data[10], data[9], data[8], data[7]
        ));
        // 5. manufacturer, 2 byte, LSB first -> 3 letters of 5 bits each
        let manuf_code = (data[12] as u32) << 8 | data[11] as u32;
        let manufacturer: String = [10u32, 5, 0]
            .iter()
            .map(|shift| char::from_u32(((manuf_code >> shift) & 0x001F) + 64).unwrap_or('?'))
            .collect();
        header.manufacturer = Some(manufacturer);
        // 6. version
        header.version = Some(data[13]);
        // 7. medium as code and as text
        let medium = data[14];
        header.medium_code = Some(medium);
        header.medium = Some(
            MEDIUM_NAMES
                .iter()
                .find(|(m, _)| *m == medium)
                .map(|(_, v)| (*v).to_string())
                .unwrap_or_else(|| "Unknown".to_string()),
        );
        // 8. transmission counter
        header.access_counter = Some(data[15]);
        // 9. status byte incl. bit breakdown
        let status = data[16];
        header.status = Some(status);
        let app_error = status & 0x03;
        let status_details = StatusDetails {
            battery_low: (status & 0x04) != 0,
            permanent_error: (status & 0x08) != 0,
            temporary_error: (status & 0x10) != 0,
            // QUIRK: 0x03 (reserved) gets NO app_status key at all.
            app_status: match app_error {
                0x00 => Some("no_error"),
                0x01 => Some("busy"),
                0x02 => Some("any_error"),
                _ => None,
            },
        };
        header.status_details = Some(status_details);
        // 10. signature, 2 byte, LSB first
        header.signature = Some(((data[18] as u16) << 8) | data[17] as u16);

        Some(header)
    }

    // ------------------------------------------------------------------
    // NEW: complete telegram
    // ------------------------------------------------------------------
    /// NEW (not in C++): splits a complete M-Bus telegram apart like
    /// `test_decode.cpp`.
    pub fn decode_telegram(&mut self, raw: &[u8]) -> crate::frame::Telegram {
        use crate::frame::{Frame, Telegram};

        let data = raw;
        self.error = 0;

        let byte_count = data.len();
        let too_short = byte_count < 6;
        let mut packet_size: Option<usize> = None;
        let mut start_ok = false;
        let mut length_ok = false;
        let mut fits = false;
        let mut stop_ok = false;
        let mut checksum: Option<u32> = None;
        let mut checksum_received: Option<u32> = None;
        let mut checksum_ok = false;
        let mut quartet_checksum_ok = false;
        let mut frame_error = String::new();
        let mut error = 0u8;
        let mut header: Option<Header> = None;
        let mut records: Vec<Record> = Vec::new();

        if too_short {
            frame_error = format!(
                "Telegram too short: {} bytes, at least 6 required.",
                byte_count
            );
            error = MbusError::BufferOverflow as u8;
        } else {
            let ps = data[1] as usize + 6;
            packet_size = Some(ps);
            start_ok = data[0] == 0x68 && data[3] == 0x68;
            length_ok = data[1] == data[2];
            fits = ps <= byte_count;
            if fits {
                let sum: u32 = data[4..ps - 2].iter().map(|&b| b as u32).sum();
                checksum = Some(sum & 0xFF);
                checksum_received = Some(data[ps - 2] as u32);
                checksum_ok = checksum_received == checksum;
                let mut quartet: u32 = 0;
                for &byte in &data[4..ps - 2] {
                    quartet += (byte & 0x0F) as u32 + (byte >> 4) as u32;
                }
                quartet_checksum_ok = checksum_received == Some(quartet & 0xFF);
                stop_ok = data[ps - 1] == 0x16;
            }

            if !start_ok {
                frame_error = format!(
                    "Unexpected start byte: 0x{:02X}/0x{:02X} instead of 0x68/0x68.",
                    data[0], data[3]
                );
            } else if !length_ok {
                frame_error = format!(
                    "Length fields differ: L1=0x{:02X}, L2=0x{:02X}.",
                    data[1], data[2]
                );
            } else if !fits {
                frame_error = format!(
                    "Declared length {} expects {} bytes, but only {} bytes present.",
                    data[1], ps, byte_count
                );
                error = MbusError::BufferOverflow as u8;
            } else if !stop_ok {
                frame_error = format!("Stop byte missing: 0x{:02X} instead of 0x16.", data[ps - 1]);
            } else if !checksum_ok && !quartet_checksum_ok {
                frame_error = format!(
                    "Checksum mismatch: expected 0x{:02X}, received 0x{:02X}.",
                    checksum.unwrap_or(0),
                    checksum_received.unwrap_or(0)
                );
            }

            if fits {
                header = self.decode_header(data, Some(ps));
                let start_address = 0x13;
                let data_len = ps.saturating_sub(start_address).saturating_sub(2);
                let payload: &[u8] = if ps > start_address {
                    &data[start_address..ps]
                } else {
                    &[]
                };
                records = self.decode_records(payload, Some(data_len));
                error = self.error;
            }
        }

        let error_message = if error != 0 {
            error_message(error)
        } else if !frame_error.is_empty() {
            frame_error.clone()
        } else {
            "No error".to_string()
        };

        let frame = Frame {
            byte_count,
            packet_size,
            // `.get()` instead of `[]`: the Python port indexes `data[6]`
            // directly and crashes on a 6-byte input; this stays runnable
            // (a 6-byte frame has no CI field anyway).
            slave_address: if too_short {
                None
            } else {
                data.get(5).copied()
            },
            c_field: if too_short {
                None
            } else {
                data.get(4).copied()
            },
            ci_field: if too_short {
                None
            } else {
                data.get(6).copied()
            },
            start_ok,
            length_ok,
            fits,
            stop_ok,
            checksum,
            checksum_received,
            checksum_ok,
            quartet_checksum_ok,
            ok: !too_short
                && start_ok
                && length_ok
                && fits
                && stop_ok
                && (checksum_ok || quartet_checksum_ok),
            error_message: frame_error,
        };

        Telegram {
            frame,
            header,
            records,
            error,
            error_message,
        }
    }
}

// ===========================================================================
// decodeRecords() core (free function so the error state stays explicit)
// ===========================================================================
fn decode_records_inner(data: &[u8], size: usize) -> (Vec<Record>, u8) {
    let buf_len = data.len() as isize;
    let size = size as isize;
    let mut records: Vec<Record> = Vec::new();
    let mut error: u8 = 0;

    macro_rules! overflow {
        () => {{
            error = 1;
            return (records, error);
        }};
    }
    macro_rules! read {
        ($p:expr) => {
            match rd(data, $p) {
                Ok(v) => v,
                Err(_) => overflow!(),
            }
        };
    }

    let mut index: isize = 0;

    while index < size {
        // ---------------------------------------------------------------
        // decode the DIF
        // ---------------------------------------------------------------
        let dif = read!(index);
        index += 1;
        let dif_least4bit = (dif & 0x0F) as usize;
        let dif_function_field = ((dif & 0x30) >> 4) as usize;

        let (mut length, mut data_coding_type) = DIF_TABLE[dif_least4bit];
        let function_field = FUNCTION_FIELDS[dif_function_field];

        // ---------------------------------------------------------------
        // handle the DIFEs
        // ---------------------------------------------------------------
        let mut storage_number: u64 = if (dif & 0x40) == 0x40 { 1 } else { 0 };
        let mut dife_number: usize = 0;
        let mut dife = [0u8; 11];
        let mut if_dife = (dif & 0x80) == 0x80;
        while if_dife {
            dife_number += 1;
            if dife_number > 9 {
                overflow!();
            }
            dife[dife_number] = read!(index);
            if_dife = (read!(index) & 0x80) == 0x80;
            index += 1;
        }

        // evaluate the DIFEs
        // Python keeps arbitrary precision here (no uint8 truncation), so
        // u64 is used instead of the C++ uint8_t to stay 1:1 with the port.
        let mut sub_unit: u64 = 0;
        let mut tariff: u64 = 0;
        let mut i: usize = 0;
        while dife_number > 0 && i <= dife_number {
            if i == 0 {
                storage_number |= ((dife[i + 1] & 0x0F) as u64) << 1;
            } else {
                storage_number |= ((dife[i + 1] & 0x0F) as u64) << (4 * i);
            }
            sub_unit |= (((dife[i + 1] & 0x40) >> 6) as u64) << i;
            tariff |= (((dife[i + 1] & 0x30) >> 4) as u64) << (2 * i);
            i += 1;
        }

        // ---------------------------------------------------------------
        // fetch the VIF(E)
        // ---------------------------------------------------------------
        let mut vif: u32 = 0;
        let mut vifarray = [0u8; 10];
        let mut vifcounter: usize = 0;
        let mut custom_vif_len: usize = 0;
        let mut custom_vif = String::new();
        let mut if_custom_vif = false;

        loop {
            if index == size {
                error = MbusError::BufferOverflow as u8;
                return (records, error);
            }
            if vifcounter > 9 {
                overflow!();
            }
            vifarray[vifcounter] = read!(index);
            index += 1;

            if (vifarray[0] & 0x7F) == 0x7C && vifcounter == 0 {
                custom_vif_len = read!(index) as usize;
                if vifarray[0] == 0xFC {
                    index = index + custom_vif_len as isize + 1;
                }
            }

            if vifcounter < 2 {
                vif = (vif << 8) + vifarray[vifcounter] as u32;
            } else if vifcounter == 2 && vif == 0xFDFD {
                vif = (vif << 8) + vifarray[vifcounter] as u32;
            }

            vifcounter += 1;
            if (vifarray[vifcounter - 1] & 0x80) != 0x80 {
                break;
            }
        }

        // fine-tune the VIF
        if (vifarray[0] & 0x80) == 0x80
            && vifarray[0] != 0xFD
            && vifarray[0] != 0xFC
            && vifarray[0] != 0xFB
            && vifarray[0] != 0xFF
        {
            vif = (vifarray[0] & 0x7F) as u32;
        }

        if vifarray[0] == 0x7C {
            vif = 0x7C00;
        }

        if (vif & 0x7F) == 0x6D {
            data_coding_type = 6;
        } else if (vif & 0x7F) == 0x6C {
            data_coding_type = 7;
        } else if (vif & 0x7F00) == 0x7C00 {
            vif = 0xFC00;
            if vifarray[0] == 0xFC {
                index = index - custom_vif_len as isize - 1;
            }
            let mut vif_buffer = vec![0u8; custom_vif_len + 1];
            for i in 0..(custom_vif_len + 1) {
                vif_buffer[custom_vif_len - i] = read!(index - vifcounter as isize + 1);
                index += 1;
            }
            custom_vif = latin1(&vif_buffer[..custom_vif_len.min(19)]);
            if_custom_vif = true;
        }

        // clear bit 7 of the last VIF byte so _findDefinition matches
        vif &= 0xFFFFFF7F;

        // ---------------------------------------------------------------
        // look up the definition
        // ---------------------------------------------------------------
        let def_index = match find_definition(vif) {
            Some(idx) => idx,
            None => {
                // C++ sets the error and then CONTINUES with entry 0.
                error = MbusError::UnsupportedVif as u8;
                0
            }
        };

        // ---------------------------------------------------------------
        // VIFE extensions
        // ---------------------------------------------------------------
        let mut extension_scaler: i32 = 0;
        let mut extension_additive_constant: f64 = 0.0;
        let mut name_extension = String::new();
        let mut no_unit = false;

        if vifcounter as isize - 1 > 0 {
            let first_vife_extension: usize = if vifarray[0] == 0xFB
                || (vifarray[0] == 0xFD && vifarray[1] != 0xFD)
                || vifarray[0] == 0xFF
            {
                2
            } else if vifarray[0] == 0xFD && vifarray[1] == 0xFD {
                3
            } else {
                1
            };

            let mut extensions_counter = first_vife_extension;
            loop {
                if extensions_counter > 9 {
                    overflow!();
                }
                let byte = vifarray[extensions_counter];

                if (byte & 0x7F) == 0x7D {
                    // E111 1101: multiplicative correction factor 1000
                    extension_scaler = 3;
                } else if (byte & 0x78) == 0x70 {
                    // E111 0nnn: multiplicative factor 10^(nnn-6)
                    extension_scaler = (byte & 0x07) as i32 - 6;
                } else if (byte & 0x7C) == 0x78 {
                    // E111 10nn: additive correction constant 10^(nn-3)
                    let additive_scaler = (byte & 0x03) as i32 - 3;
                    extension_additive_constant = 1.0;
                    for _ in 0..additive_scaler {
                        extension_additive_constant *= 10.0;
                    }
                    for _ in 0..(-additive_scaler).max(0) {
                        extension_additive_constant /= 10.0;
                    }
                } else if (byte & 0x7A) == 0x6A {
                    // E110 1f1b: date/time - adapt the coding
                    if dif_least4bit == 4 {
                        data_coding_type = 6;
                    } else if dif_least4bit == 2 {
                        data_coding_type = 7;
                    }
                    name_extension = "_TimeSt".to_string();
                    no_unit = true;
                } else if byte == 0xFC || byte == 0xFF {
                    // "reading user" extension: the NEXT byte selects phase
                    if extensions_counter + 1 > 9 {
                        overflow!();
                    }
                    let extension_buffer = (vifarray[extensions_counter + 1] & 0x7F) as u32;
                    if let Some(suffix) = phase_suffix(extension_buffer) {
                        name_extension = suffix.to_string();
                        extensions_counter += 1;
                    }
                }
                extensions_counter += 1;
                if extensions_counter > vifcounter {
                    break;
                }
            }
        }

        // ---------------------------------------------------------------
        // buffer overflow check
        // ---------------------------------------------------------------
        if index + length as isize > size || index + length as isize > buf_len {
            error = MbusError::BufferOverflow as u8;
            return (records, error);
        }

        // ---------------------------------------------------------------
        // read the value
        // ---------------------------------------------------------------
        let mut value16: u32 = 0;
        let mut value32: u32 = 0;
        // Exact integer value: i128 holds every C++ value (incl. 64-bit
        // unsigned) and converts to f64 with correct rounding.
        let mut value_int: i128 = 0;
        let mut value_float: f32 = 0.0;
        let mut value_date: f64 = 0.0;
        let mut is_date_value = false;
        let mut value_string = String::new();
        let mut negative = false;
        let mut ascii_value: u8 = 0;

        loop {
            let mut switch_again = false;
            match data_coding_type {
                0 => {
                    // no data, value stays 0
                }
                1 => {
                    // integer, LITTLE-ENDIAN
                    if length == 2 {
                        value16 = 0;
                        for i in 0..length {
                            value16 = (value16 << 8)
                                + read!(index + length as isize - i as isize - 1) as u32;
                        }
                        value_int = to_int16(value16 as i64) as i128;
                    } else if length == 4 {
                        value32 = 0;
                        for i in 0..length {
                            value32 = (value32 << 8)
                                + read!(index + length as isize - i as isize - 1) as u32;
                        }
                        value_int = to_int32(value32 as i64) as i128;
                    } else {
                        value_int = 0;
                        for i in 0..length {
                            value_int = (value_int << 8)
                                + read!(index + length as isize - i as isize - 1) as i128;
                        }
                    }
                }
                2 => {
                    // BCD, 2 digits per byte
                    if length == 2 {
                        value16 = 0;
                    } else if length == 4 {
                        value32 = 0;
                    } else {
                        value_int = 0;
                    }
                    for i in 0..length {
                        let mut byte = read!(index + length as isize - i as isize - 1);
                        if i == 0 && (byte & 0xF0) == 0xF0 {
                            // top half byte = "negative" marker; QUIRK: the
                            // tens digit of that byte is lost as well.
                            byte &= 0x0F;
                            negative = true;
                        }
                        let digit = ((byte >> 4) as u32) * 10 + (byte & 0x0F) as u32;
                        if length == 2 {
                            value16 = value16 * 100 + digit;
                        } else if length == 4 {
                            value32 = value32 * 100 + digit;
                        } else {
                            value_int = value_int * 100 + digit as i128;
                        }
                    }
                    if length == 2 {
                        value_int = to_int16(value16 as i64) as i128;
                    } else if length == 4 {
                        value_int = to_int32(value32 as i64) as i128;
                    }
                    if negative {
                        value_int = -value_int;
                    }
                }
                3 => {
                    // real = IEEE-754 32 bit
                    value_int = 0;
                    for i in 0..length {
                        value_int = (value_int << 8)
                            + read!(index + length as isize - i as isize - 1) as i128;
                    }
                    value_float = f32::from_bits((value_int & 0xFFFF_FFFF) as u32);
                }
                4 => {
                    // variable length, LVAR
                    let lvar = read!(index);
                    if lvar <= 0xBF {
                        // ASCII string with LVAR characters
                        length = lvar as usize;
                        index += 1;
                        let mut char_buffer = vec![0u8; length + 1];
                        for i in 0..(length + 1) {
                            char_buffer[i] = read!(index + length as isize - i as isize - 1);
                        }
                        value_string = latin1(&char_buffer[..length.min(29)]);
                        ascii_value = 1;
                    } else if (0xC0..=0xCF).contains(&lvar) {
                        length = (lvar - 0xC0) as usize;
                        index += 1;
                        data_coding_type = 2;
                        switch_again = true;
                    } else if (0xD0..=0xDF).contains(&lvar) {
                        length = (lvar - 0xD0) as usize;
                        index += 1;
                        data_coding_type = 2;
                        switch_again = true;
                        negative = true;
                    } else if (0xE0..=0xEF).contains(&lvar) {
                        length = (lvar - 0xE0) as usize;
                        index += 1;
                        data_coding_type = 1;
                        switch_again = true;
                    } else if (0xF0..=0xFA).contains(&lvar) {
                        length = (lvar - 0xF0) as usize;
                        index += 1;
                        data_coding_type = 3;
                        switch_again = true;
                    }
                }
                5 => {
                    // special function, nothing to do
                }
                6 => {
                    // date+time type F, 4 bytes
                    let start = index as usize;
                    let date_slice = &data[start..start + length];
                    if date_slice.len() < 4 {
                        overflow!();
                    }
                    if (date_slice[0] & 0x80) == 0 {
                        let year =
                            (((date_slice[2] & 0xE0) >> 5) | ((date_slice[3] & 0xF0) >> 1)) as u32;
                        let month = (date_slice[3] & 0x0F) as u32;
                        let mday = (date_slice[2] & 0x1F) as u32;
                        let hour = (date_slice[1] & 0x1F) as u32;
                        let minute = (date_slice[0] & 0x3F) as u32;
                        let datestring = format!(
                            "20{:02}{:02}{:02}{:02}{:02}",
                            year, month, mday, hour, minute
                        );
                        let datestring2 = format!(
                            "20{:02}-{:02}-{:02} {:02}:{:02}:00",
                            year, month, mday, hour, minute
                        );
                        value_date = datestring.parse::<f64>().unwrap_or(0.0);
                        is_date_value = true;
                        value_string = datestring2;
                        ascii_value = 2;
                    }
                }
                7 => {
                    // date type G, 2 bytes
                    let start = index as usize;
                    let date_slice = &data[start..start + length];
                    if date_slice.len() < 2 {
                        overflow!();
                    }
                    if (date_slice[1] & 0x0F) <= 12 {
                        let year =
                            (((date_slice[0] & 0xE0) >> 5) | ((date_slice[1] & 0xF0) >> 1)) as u32;
                        let month = (date_slice[1] & 0x0F) as u32;
                        let mday = (date_slice[0] & 0x1F) as u32;
                        let datestring = format!("20{:02}{:02}{:02}", year, month, mday);
                        let datestring2 = format!("20{:02}-{:02}-{:02}", year, month, mday);
                        value_date = datestring.parse::<f64>().unwrap_or(0.0);
                        is_date_value = true;
                        value_string = datestring2;
                        ascii_value = 2;
                    }
                }
                _ => {}
            }

            if !switch_again {
                break;
            }
        }

        // index += len;
        index += length as isize;

        // ---------------------------------------------------------------
        // scaling
        // ---------------------------------------------------------------
        let mut scalar: i32 = 0;
        if def_index != 0 {
            let (_code, base, _size, def_scalar) = VIF_DEFS[def_index];
            scalar = to_int8(def_scalar as i64 + vif as i64 - base as i64 + extension_scaler as i64)
                as i32;
        }

        let base_value: f64 = if data_coding_type == 3 {
            value_float as f64
        } else if is_date_value {
            value_date
        } else {
            value_int as f64
        };

        let scaled: f64;
        if data_coding_type == 3 {
            let mut s = base_value;
            if vifarray[0] != 0xFF {
                for _ in 0..scalar.max(0) {
                    s *= 10.0;
                }
                for _ in 0..(-scalar).max(0) {
                    s /= 10.0;
                }
                s += extension_additive_constant;
            }
            scaled = s;
        } else if vifarray[0] == 0xFF {
            // manufacturer specific: do NOT scale, do NOT add
            scaled = base_value;
        } else {
            let mut s = base_value;
            for _ in 0..scalar.max(0) {
                s *= 10.0;
            }
            for _ in 0..(-scalar).max(0) {
                s /= 10.0;
            }
            s += extension_additive_constant;
            scaled = s;
        }

        // ---------------------------------------------------------------
        // build the record
        // ---------------------------------------------------------------
        let code = VIF_DEFS[def_index].0;
        let mut record = Record {
            vif: format!("0x{:x}", vif),
            code,
            name: format!("{}{}{}", code_name(code), name_extension, function_field),
            ..Default::default()
        };

        if ascii_value != 1 {
            record.value_scaled = Some(scaled);
        }
        if ascii_value > 0 {
            record.value_string = Some(value_string.clone());
        }
        if if_custom_vif {
            record.units = Some(custom_vif.clone());
        } else if let Some(unit) = code_units(code) {
            if !no_unit {
                record.units = Some(unit.to_string());
            }
        }
        if sub_unit > 0 {
            record.sub_unit = Some(sub_unit);
        }
        if storage_number > 0 {
            record.storage = Some(storage_number);
        }
        if tariff > 0 {
            record.tariff = Some(tariff);
        }

        records.push(record);

        // ---------------------------------------------------------------
        // trailing markers
        // ---------------------------------------------------------------
        let following = if index < buf_len {
            rd(data, index).ok()
        } else {
            None
        };
        if following == Some(0x0F) && index != size {
            break;
        }
        if following == Some(0x1F) && index != size {
            if let Some(last) = records.last_mut() {
                last.telegram_follow = Some(1);
            }
            break;
        }
    }

    (records, error)
}

/// Convenience: parse a hex string ("68 56 ...", commas and `0x` allowed).
pub fn parse_hex(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for token in text.replace(',', " ").split_whitespace() {
        let token = token.trim();
        let token = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
            .unwrap_or(token);
        if token.is_empty() {
            continue;
        }
        if let Ok(value) = u32::from_str_radix(token, 16) {
            out.push((value & 0xFF) as u8);
        }
    }
    out
}
