//! Golden tests against the reference output of the **unmodified C++
//! `MBusinoLib`**.
//!
//! The same comparison rules as the Python port's `tests/helpers.py` are used:
//!
//! * `name`, `units`, `value_string`, `vif`, `code`, `subUnit`, `storage`,
//!   `tariff`, `telegramFollow` must match **exactly**, and so must the key
//!   set of every record (ArduinoJson only keeps keys that are present).
//! * `value_scaled` is compared with `rel_tol = 1e-6`, `abs_tol = 1e-6`
//!   (`ARDUINO_FLOAT_DECIMALS = 6`), because ArduinoJson rounds while
//!   serialising.
//!
//! The golden files under `tests/golden/` are copied verbatim from
//! `MBusinoLibPy/tests/golden/`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use mbusino_lib_rs::{parse_hex, Decoder, Header, Record, VALUE_ABS_TOL, VALUE_REL_TOL};
use serde_json::{json, Value};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn read_text(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e))
}

fn load_json(path: &Path) -> Value {
    serde_json::from_str(&read_text(path))
        .unwrap_or_else(|e| panic!("invalid json {}: {}", path.display(), e))
}

fn read_hex_file(path: &Path) -> Vec<u8> {
    parse_hex(&read_text(path))
}

/// Record -> JSON `Value` (round-tripped through our own serialiser).
fn record_value(record: &Record) -> Value {
    serde_json::from_str(&record.to_json_string()).unwrap()
}

fn header_value(header: &Header) -> Value {
    serde_json::from_str(&header.to_json_string()).unwrap()
}

fn isclose(a: f64, b: f64) -> bool {
    let diff = (a - b).abs();
    let rel = VALUE_REL_TOL * a.abs().max(b.abs());
    diff <= rel.max(VALUE_ABS_TOL)
}

fn key_set(value: &Value) -> BTreeSet<&str> {
    value
        .as_object()
        .map(|m| m.keys().map(|k| k.as_str()).collect())
        .unwrap_or_default()
}

/// Compare two record lists following the `helpers.py` rules.
fn compare_records(got: &[Value], want: &[Value], label: &str) -> Vec<String> {
    let mut problems = Vec::new();
    if got.len() != want.len() {
        problems.push(format!(
            "{}: count {} instead of {}",
            label,
            got.len(),
            want.len()
        ));
    }
    for (index, (g_item, w_item)) in got.iter().zip(want.iter()).enumerate() {
        let where_ = format!("{}[{}]", label, index);
        let g_keys = key_set(g_item);
        let w_keys = key_set(w_item);
        if g_keys != w_keys {
            problems.push(format!(
                "{}: keys differ: only here {:?} / only there {:?}",
                where_, g_keys, w_keys
            ));
            continue;
        }
        let g_obj = g_item.as_object().unwrap();
        for (key, w_val) in w_item.as_object().unwrap() {
            let g_val = &g_obj[key];
            if key == "value_scaled" {
                let ga = g_val.as_f64().unwrap_or(f64::NAN);
                let wa = w_val.as_f64().unwrap_or(f64::NAN);
                if !isclose(ga, wa) {
                    problems.push(format!(
                        "{}: value_scaled {} != {} (diff {:e})",
                        where_,
                        ga,
                        wa,
                        (ga - wa).abs()
                    ));
                }
            } else if g_val != w_val {
                problems.push(format!("{}: {} {} != {}", where_, key, g_val, w_val));
            }
        }
    }
    problems
}

/// Exactly the flow of `test_decode.cpp` / `records_like_test_decode()`.
fn records_like_test_decode(data: &[u8]) -> (Vec<Value>, u8) {
    let packet_size = data[1] as usize + 6;
    let start_address = 0x13;
    let data_len = packet_size - start_address - 2;
    let mut decoder = Decoder::new();
    let records = decoder.decode_records(&data[start_address..packet_size], Some(data_len));
    let error = decoder.get_error();
    (records.iter().map(record_value).collect(), error)
}

fn sorted_dir(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", dir.display(), e))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    entries
}

// ===========================================================================
// Records
// ===========================================================================
#[test]
fn golden_records_match() {
    let dir = golden_dir().join("records");
    let mut problems = Vec::new();
    let mut count = 0;
    for path in sorted_dir(&dir) {
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let case = load_json(&path);
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let data = parse_hex(case["hex"].as_str().unwrap());
        let mut decoder = Decoder::new();
        let records = decoder.decode_records(&data, None);
        let error = decoder.get_error();
        let got: Vec<Value> = records.iter().map(record_value).collect();
        let want = case["records"].as_array().unwrap();

        let expected_error = case["error_code"].as_u64().unwrap() as u8;
        if error != expected_error {
            problems.push(format!(
                "{}: error {} instead of {}",
                name, error, expected_error
            ));
        }
        problems.extend(compare_records(&got, want, &name));
        count += 1;
    }
    assert!(count > 0, "no record goldens found");
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn aunit_cases_covered() {
    let dir = golden_dir().join("records");
    let names: BTreeSet<String> = sorted_dir(&dir)
        .iter()
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
        .collect();
    for expected in [
        "aunit_number_1",
        "aunit_number_2",
        "aunit_number_3",
        "aunit_number_4",
        "aunit_number_6",
        "aunit_multifield",
        "aunit_decode_2bcd",
        "aunit_decode_8bcd",
    ] {
        assert!(names.contains(expected), "missing aunit case {}", expected);
    }
}

#[test]
fn cpp_return_value_vs_json_array() {
    // QUIRK: on error decodeRecords() returns 0 in C++, but the JsonArray
    // stays partially filled. Python/Rust return the JsonArray content.
    let case = load_json(&golden_dir().join("records/konstrukt_var_bcd.json"));
    let data = parse_hex(case["hex"].as_str().unwrap());
    let mut decoder = Decoder::new();
    let records = decoder.decode_records(&data, None);
    assert_eq!(decoder.get_error(), 1, "expected BUFFER_OVERFLOW");
    assert_eq!(case["records_decoded"].as_u64().unwrap(), 0);
    assert_eq!(records.len(), case["records"].as_array().unwrap().len());
    assert_eq!(records.len(), 1);
}

#[test]
fn unsupported_vif_sets_error() {
    // 0x6E = HCA - commented out in vif_defs -> no match -> UNSUPPORTED_VIF.
    let mut decoder = Decoder::new();
    let records = decoder.decode_records(&[0x01, 0x6E, 0x2A], None);
    assert_eq!(decoder.get_error(), 4, "expected UNSUPPORTED_VIF");
    assert_eq!(records[0].name, "unknown_vif");
    assert_eq!(records[0].code, 0);
    assert_eq!(records[0].vif, "0x6e");
    assert_eq!(records[0].value_scaled, Some(42.0));
    assert!(records[0].units.is_none());
}

#[test]
fn buffer_overflow_is_reported() {
    // DIF 0x07 = 64-bit integer, but only 1 byte of data present.
    let mut decoder = Decoder::new();
    let records = decoder.decode_records(&[0x07, 0x00, 0x11], None);
    assert_eq!(decoder.get_error(), 1);
    assert!(records.is_empty());
}

#[test]
fn buffer_overflow_keeps_already_decoded_records() {
    let mut decoder = Decoder::new();
    let records = decoder.decode_records(&[0x01, 0x00, 0x2A, 0x07, 0x00], None);
    assert_eq!(decoder.get_error(), 1);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].value_scaled, Some(0.042));
}

// ===========================================================================
// Telegrams
// ===========================================================================
fn expected_telegrams() -> Vec<(String, Value)> {
    sorted_dir(&golden_dir())
        .into_iter()
        .filter(|p| p.to_string_lossy().ends_with(".expected.json"))
        .map(|p| {
            let name = p
                .file_name()
                .unwrap()
                .to_string_lossy()
                .trim_end_matches(".expected.json")
                .to_string();
            (name, load_json(&p))
        })
        .collect()
}

#[test]
fn at_least_five_golden_telegrams() {
    assert!(expected_telegrams().len() >= 5);
}

#[test]
fn every_golden_has_documented_source() {
    for (name, _) in expected_telegrams() {
        let source = load_json(&golden_dir().join(format!("{}.source.json", name)));
        assert!(
            source["src"]
                .as_str()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false),
            "source missing for {}",
            name
        );
        assert!(
            source["note"]
                .as_str()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false),
            "provenance note missing for {}",
            name
        );
    }
}

#[test]
fn telegram_records_match_cpp_reference() {
    let mut problems = Vec::new();
    for (name, expected) in expected_telegrams() {
        let data = read_hex_file(&golden_dir().join(format!("{}.hex", name)));
        let (got, error) = records_like_test_decode(&data);

        let expected_error = expected["error_code"].as_u64().unwrap() as u8;
        if error != expected_error {
            problems.push(format!(
                "{}: error {} instead of {}",
                name, error, expected_error
            ));
        }
        let expected_count = expected["records_decoded"].as_u64().unwrap() as usize;
        if got.len() != expected_count {
            problems.push(format!(
                "{}: {} records instead of {}",
                name,
                got.len(),
                expected_count
            ));
        }
        problems.extend(compare_records(
            &got,
            expected["records"].as_array().unwrap(),
            &name,
        ));
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn record_keys_match_arduinojson_output() {
    let allowed: BTreeSet<&str> = [
        "vif",
        "code",
        "value_scaled",
        "value_string",
        "units",
        "name",
        "subUnit",
        "storage",
        "tariff",
        "telegramFollow",
    ]
    .into_iter()
    .collect();
    let mut found: BTreeSet<String> = BTreeSet::new();
    for (name, expected) in expected_telegrams() {
        for record in expected["records"].as_array().unwrap() {
            for key in record.as_object().unwrap().keys() {
                assert!(
                    allowed.contains(key.as_str()),
                    "unexpected key {} in {}",
                    key,
                    name
                );
                found.insert(key.clone());
            }
        }
    }
    for expected in [
        "vif",
        "code",
        "value_scaled",
        "name",
        "units",
        "value_string",
        "subUnit",
        "storage",
    ] {
        assert!(
            found.contains(expected),
            "key {} does not appear in any golden telegram",
            expected
        );
    }
}

#[test]
fn known_values_from_reference_telegram() {
    let data = read_hex_file(&golden_dir().join("efe_heat_meter.hex"));
    let (records, _error) = records_like_test_decode(&data);
    assert_eq!(records[0]["name"], "fab_number");
    assert_eq!(records[0]["vif"], "0x78");
    assert!((records[0]["value_scaled"].as_f64().unwrap() - 73753409.0).abs() < 1e-3);
    // 0x03 (inside the ENERGY_WH definition) -> table index 1, because
    // UNKNOWN_VIF sits at position 0.
    assert_eq!(mbusino_lib_rs::find_definition(0x03), Some(1));
}

#[test]
fn no_decode_errors_in_golden_set() {
    for (name, expected) in expected_telegrams() {
        assert_eq!(expected["error_code"].as_u64().unwrap(), 0, "{}", name);
        let data = read_hex_file(&golden_dir().join(format!("{}.hex", name)));
        let (_got, error) = records_like_test_decode(&data);
        assert_eq!(error, 0, "{}", name);
    }
}

// ===========================================================================
// Header
// ===========================================================================
#[test]
fn golden_headers_match() {
    let mut problems = Vec::new();
    let mut ok_cases = 0;
    for path in sorted_dir(&golden_dir()) {
        if !path.to_string_lossy().ends_with(".header.json") {
            continue;
        }
        let case = load_json(&path);
        let name = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .trim_end_matches(".header.json")
            .to_string();
        let data = parse_hex(case["hex"].as_str().unwrap());
        let packet_size = data[1] as usize + 6;
        let decoder = Decoder::new();
        let header = decoder.decode_header(&data, Some(packet_size));

        let ok = header.as_ref().map(|h| h.is_ok()).unwrap_or(false);
        let expected_ok = case["header_ok"].as_bool().unwrap();
        if ok != expected_ok {
            problems.push(format!(
                "{}: header_ok {} instead of {}",
                name, ok, expected_ok
            ));
        }
        let value = header
            .map(|h| header_value(&h))
            .unwrap_or_else(|| json!({}));
        if value != case["header"] {
            problems.push(format!(
                "{}: header fields differ:\n got {}\n want {}",
                name, value, case["header"]
            ));
        }
        if expected_ok {
            ok_cases += 1;
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
    assert!(ok_cases >= 5, "expected at least 5 positive header cases");
}

#[test]
fn header_status_bit_decode() {
    let data = read_hex_file(&golden_dir().join("efe_heat_meter.hex"));
    let decoder = Decoder::new();

    let mut modified = data.clone();
    modified[16] = 0x04 | 0x08 | 0x10 | 0x01;
    let header = decoder
        .decode_header(&modified, Some(modified.len()))
        .unwrap();
    let details = header.status_details.unwrap();
    assert!(details.battery_low);
    assert!(details.permanent_error);
    assert!(details.temporary_error);
    assert_eq!(details.app_status, Some("busy"));

    modified[16] = 0x03;
    let header = decoder
        .decode_header(&modified, Some(modified.len()))
        .unwrap();
    assert_eq!(header.status_details.unwrap().app_status, None);
}

#[test]
fn header_error_cases() {
    let decoder = Decoder::new();
    // 19 zeros: no 0x68 at position 0 and 3
    assert!(decoder.decode_header(&[0u8; 19], None).is_none());

    // length mismatch
    let mut data = [0u8; 19];
    data[0] = 0x68;
    data[3] = 0x68;
    data[1] = 10;
    data[2] = 11;
    assert!(decoder.decode_header(&data, None).is_none());

    // enough bytes, but length argument < 19
    assert!(decoder.decode_header(&[0u8; 30], Some(18)).is_none());

    // short buffer with a large length argument -> {} (no out-of-bounds read)
    assert!(decoder.decode_header(&[0u8; 10], Some(37)).is_none());

    // CI not 0x72/0x76 -> only the four basic fields
    let mut data = [0u8; 25];
    data[0] = 0x68;
    data[3] = 0x68;
    data[1] = 19;
    data[2] = 19;
    data[6] = 0x7A;
    let header = decoder.decode_header(&data, None).unwrap();
    assert!(!header.is_ok());
    assert_eq!(header.len, Some(19));
    assert_eq!(header.c_field, Some(0));
    assert_eq!(header.address, Some(0));
    assert_eq!(header.ci_field, Some(0x7A));
    assert!(header.id.is_none());
}

// ===========================================================================
// decode_telegram() (new, no C++ counterpart)
// ===========================================================================
fn telegram_by_name(name: &str) -> Vec<u8> {
    read_hex_file(&golden_dir().join(format!("{}.hex", name)))
}

#[test]
fn decode_telegram_happy_path() {
    for (name, expected) in expected_telegrams() {
        let data = telegram_by_name(&name);
        let mut decoder = Decoder::new();
        let result = decoder.decode_telegram(&data);
        let packet_size = data[1] as usize + 6;

        assert_eq!(result.frame.byte_count, data.len(), "{}", name);
        assert_eq!(result.frame.packet_size, Some(packet_size), "{}", name);
        assert!(result.frame.start_ok, "{}", name);
        assert!(result.frame.length_ok, "{}", name);
        assert!(result.frame.fits, "{}", name);
        assert!(result.frame.stop_ok, "{}", name);
        assert_eq!(result.frame.slave_address, Some(data[5]), "{}", name);
        assert_eq!(result.frame.c_field, Some(data[4]), "{}", name);
        assert_eq!(result.frame.ci_field, Some(data[6]), "{}", name);
        let expected_sum: u32 = data[4..packet_size - 2]
            .iter()
            .map(|&b| b as u32)
            .sum::<u32>()
            & 0xFF;
        assert_eq!(result.frame.checksum, Some(expected_sum), "{}", name);
        assert_eq!(
            result.frame.checksum_received,
            Some(data[packet_size - 2] as u32),
            "{}",
            name
        );
        assert_eq!(
            result.records.len(),
            expected["records_decoded"].as_u64().unwrap() as usize
        );
        assert_eq!(
            result.error as u64,
            expected["error_code"].as_u64().unwrap()
        );

        let header_golden = load_json(&golden_dir().join(format!("{}.header.json", name)));
        let hv = result
            .header
            .as_ref()
            .map(header_value)
            .unwrap_or_else(|| json!({}));
        assert_eq!(hv, header_golden["header"], "{}", name);
    }
}

#[test]
fn only_elster_has_a_bad_checksum() {
    let mut bad = Vec::new();
    let mut good = Vec::new();
    for (name, _) in expected_telegrams() {
        let data = telegram_by_name(&name);
        let mut decoder = Decoder::new();
        let result = decoder.decode_telegram(&data);
        if result.frame.checksum_ok {
            good.push(name);
        } else {
            bad.push(name);
        }
    }
    assert_eq!(bad, vec!["elster".to_string()]);
    assert_eq!(good.len(), 4);
}

#[test]
fn decode_telegram_clean_message() {
    let data = telegram_by_name("efe_heat_meter");
    let mut decoder = Decoder::new();
    let result = decoder.decode_telegram(&data);
    assert_eq!(result.error, 0);
    assert_eq!(result.error_message, "No error");
    assert!(result.frame.ok);
    assert_eq!(result.frame.error_message, "");
}

#[test]
fn decode_telegram_too_short() {
    let mut decoder = Decoder::new();
    let result = decoder.decode_telegram(&[0x68, 0x05]);
    assert_eq!(result.error, 1);
    assert!(!result.frame.ok);
    assert_eq!(result.frame.packet_size, None);
    assert_eq!(result.frame.slave_address, None);
    assert!(result.header.is_none());
    assert!(result.records.is_empty());
    assert!(result.frame.error_message.contains("too short"));
    assert_eq!(result.error_message, mbusino_lib_rs::error_message(1));
}

#[test]
fn decode_telegram_checksum_mismatch_but_records_decoded() {
    let data = telegram_by_name("elster");
    let mut decoder = Decoder::new();
    let result = decoder.decode_telegram(&data);
    assert!(!result.frame.checksum_ok);
    assert!(!result.frame.quartet_checksum_ok);
    assert!(!result.frame.ok);
    assert_eq!(result.frame.checksum, Some(0xCF));
    assert_eq!(result.frame.checksum_received, Some(0xCC));
    assert!(result.frame.error_message.contains("Checksum mismatch"));
    assert_eq!(result.error, 0);
    assert_eq!(result.error_message, result.frame.error_message);
    assert_eq!(result.records.len(), 2);
}

#[test]
fn decode_telegram_quartet_checksum_accepted() {
    let mut data = telegram_by_name("elster");
    let packet_size = data[1] as usize + 6;
    let mut quartet: u32 = 0;
    for &byte in &data[4..packet_size - 2] {
        quartet += (byte & 0x0F) as u32 + (byte >> 4) as u32;
    }
    data[packet_size - 2] = (quartet & 0xFF) as u8;
    let mut decoder = Decoder::new();
    let result = decoder.decode_telegram(&data);
    assert!(!result.frame.checksum_ok);
    assert!(result.frame.quartet_checksum_ok);
    assert!(result.frame.ok);
    assert_eq!(result.error_message, "No error");
}

#[test]
fn decode_telegram_declared_length_exceeds_buffer() {
    let mut data = telegram_by_name("elster");
    data[1] = 0x50;
    data[2] = 0x50;
    let mut decoder = Decoder::new();
    let result = decoder.decode_telegram(&data);
    assert!(!result.frame.fits);
    assert!(!result.frame.ok);
    assert_eq!(result.error, 1);
    assert!(result.records.is_empty());
    assert!(result.header.is_none());
    assert!(result.frame.error_message.contains("expects 86 bytes"));
}

#[test]
fn decode_telegram_error_is_reset_between_calls() {
    let mut decoder = Decoder::new();
    let mut broken = telegram_by_name("elster");
    broken[1] = 0x50;
    broken[2] = 0x50;
    decoder.decode_telegram(&broken);
    let result = decoder.decode_telegram(&telegram_by_name("efe_heat_meter"));
    assert_eq!(result.error, 0);
    assert_eq!(decoder.get_error(), 0);
}

#[test]
fn hex_string_input_matches_bytes() {
    let data = telegram_by_name("elster");
    let text: String = data.iter().map(|b| format!("0x{:02X} ", b)).collect();
    let mut decoder = Decoder::new();
    let from_text = decoder.decode_telegram(&parse_hex(&text));
    let mut decoder2 = Decoder::new();
    let from_bytes = decoder2.decode_telegram(&data);
    assert_eq!(from_text.records, from_bytes.records);
    assert_eq!(from_text.records.len(), 2);
}

// ===========================================================================
// Lookup helpers
// ===========================================================================
#[test]
fn lookup_helpers() {
    assert_eq!(mbusino_lib_rs::code_name(1), "energy");
    assert_eq!(mbusino_lib_rs::code_units(3), Some("m³"));
    assert_eq!(mbusino_lib_rs::code_units(0), None);
    assert_eq!(mbusino_lib_rs::device_class(3, 0x07), "water");
    assert_eq!(mbusino_lib_rs::device_class(3, 0x03), "gas");
    assert_eq!(mbusino_lib_rs::device_class(3, 0xFF), "volume");
    assert_eq!(mbusino_lib_rs::state_class(1), "total_increasing");
    assert_eq!(
        mbusino_lib_rs::state_class(61),
        "",
        "SPECIAL_SUPPLIER_INFO -> default"
    );
    // _getVIF is the inverse of _findDefinition for the tested combination
    assert_eq!(mbusino_lib_rs::get_vif(1, -3), 0);
}
