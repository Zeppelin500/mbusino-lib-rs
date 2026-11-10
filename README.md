# mbusino-lib-rs

A dependency-free **Rust port** of the Arduino library
[**MBusinoLib**](https://github.com/Zeppelin500/MBusinoLib) - an M-Bus
(Meterbus, EN 13757) `RSP_UD` decoder.

The library decodes the answer a slave (a meter) sends to the master. Most
M-Bus devices should be supported.

The port follows the C++ original **function by function**. The blueprint is
the verified 1:1 Python port
[**MBusinoLibPy**](https://github.com/Zeppelin500/MBusinoLibPy), whose golden
references were produced by the *unmodified* C++ library. This crate
reproduces the same output - see `tests/`.

## Features

* `Decoder` struct with an error state and `decode_records()`,
  `decode_header()` and (new) `decode_telegram()`.
* `Record` / `Header` / `Frame` / `Telegram` as plain structs with
  `Option` fields (a field is `None` when the ArduinoJson original does not
  write that key).
* The four lookup methods (`get_code_name`, `get_code_units`,
  `get_device_class`, `get_state_class`) and the VIF helpers
  (`find_definition`, `get_vif`).
* Lookup tables generated from the Python port (`tools/gen_tables.py`), not
  re-typed by hand - `src/tables.rs` is generated.
* **No runtime dependencies.** JSON output is provided by a small built-in
  writer (`Json`), not by an external crate.
* Numerics are bit-identical to the C++: `int8_t` / `int16_t` / `int32_t`
  wrap-around, `int8_t` truncation of the scalar, 32-bit `float` for "real"
  records. Only ArduinoJson's *output* rounds, so `value_scaled` is compared
  with a tolerance (relative `1e-6` / absolute `1e-6`,
  `ARDUINO_FLOAT_DECIMALS = 6`).

## Usage

```rust
use mbusino_lib_rs::{parse_hex, Decoder};

fn main() {
    // A complete RSP_UD telegram (EFE heat meter, fixed header only).
    let data = parse_hex("68 0F 0F 68 08 00 72 09 34 75 73 C5 14 00 0D 43 00 00 00 C8 16");

    let mut decoder = Decoder::new();
    let result = decoder.decode_telegram(&data);

    println!("{}", result.frame.ok);            // true
    println!("{:?}", result.header.unwrap().id); // Some("73753409")
    for record in &result.records {
        println!("{}", record.to_json_string());
    }
}
```

Lower-level, exactly like `test_decode.cpp` / the C++ API:

```rust
use mbusino_lib_rs::Decoder;

let packet_size = data[1] as usize + 6;
let records = decoder.decode_records(&data[0x13..packet_size], Some(packet_size - 0x13 - 2));
let error = decoder.get_error(); // returns AND clears, like getError()
```

### Examples

```sh
cargo run --example decode  -- ./tests/golden/efe_heat_meter.hex   # full telegram -> JSON
cargo run --example records -- "01 13 39"                          # raw record buffer -> JSON
```

## JSON output

Records serialise with the exact ArduinoJson key names of the original and
only the keys that are present:

`vif`, `code`, `value_scaled`, `value_string`, `units`, `name`, `subUnit`,
`storage`, `tariff`, `telegramFollow`.

## Tests

```sh
cargo test
```

The golden files under `tests/golden/` are copied verbatim from
`MBusinoLibPy/tests/golden/` and were produced by the unmodified C++
library. The tests cover:

* all record goldens (`records/*.json`), including the `aunit_*` vectors
  from the original `test.cpp`,
* all five real telegrams and their header goldens (`*.expected.json`,
  `*.header.json`, incl. the four negative header cases),
* the `value_scaled` tolerance and the exact key set,
* the error paths (`UNSUPPORTED_VIF`, `BUFFER_OVERFLOW`, partial arrays),
* the new `decode_telegram()` frame checks.

The comparison rules are the same as `MBusinoLibPy/tests/helpers.py`
(exact match for `name`/`units`/`value_string`/`vif`/`code`/`subUnit`/
`storage`/`tariff`/`telegramFollow` and the key set, tolerance for
`value_scaled`).

## Generating the tables

```sh
python3 tools/gen_tables.py   # rewrites src/tables.rs from MBusinoLibPy
```

## Origin and credits

* Original C++ library: [**MBusinoLib**](https://github.com/Zeppelin500/MBusinoLib)
  by **Zeppelin500** (based on the AllWize decoder work).
* M-Bus → MQTT gateway: [**MBusino**](https://github.com/Zeppelin500/MBusino)
  by Zeppelin500.
* Python port (blueprint for this crate):
  [**MBusinoLibPy**](https://github.com/Zeppelin500/MBusinoLibPy).

## License

**GPL-3.0-or-later** - same license as the C++ original. See [`LICENSE`](LICENSE).
