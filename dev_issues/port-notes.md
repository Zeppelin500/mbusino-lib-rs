# Development notes - mbusino-lib-rs port

Internal notes from porting **MBusinoLib 0.10.0** to Rust. This file is for
us, the developers - not for end users. The user documentation lives in the
[root README](../README.md); the folder index is [README.md](README.md) and
the list of still-open issues found while porting is
[open-points.md](open-points.md).

**`mbusino-lib-rs`** is a function-by-function Rust port of the Arduino
library [**MBusinoLib**](https://github.com/Zeppelin500/MBusinoLib)
(version 0.10.0, GPL-3.0). It decodes M-Bus (Meterbus, EN 13757) `RSP_UD`
telegrams - the answer a slave sends to the master.

The **blueprint is not the C++ directly** but the verified 1:1 Python port
[**MBusinoLibPy**](https://github.com/Zeppelin500/MBusinoLibPy): that port
already flattens the C++ UB into well-defined behaviour and documents each
function with its C++ origin, which made the Rust translation mechanical and
reviewable.

* **No runtime dependencies** - `std` only. JSON output is a small built-in
  writer ([`Json`](../src/json.rs)), not `serde`.
* **Byte-for-byte value parity** with the C++ decoder, proven by golden tests
  (see [Testing](#testing)) plus a fuzz cross-check against MBusinoLibPy.
* `#![forbid(unsafe_code)]`.

## Module map

| File | Content |
|------|---------|
| [`src/lib.rs`](../src/lib.rs) | Public re-exports, crate docs, `VERSION`. |
| [`src/decoder.rs`](../src/decoder.rs) | The `Decoder` struct: `decode_records`, `decode_header`, `decode_telegram`; the lookup helpers `code_name`, `code_units`, `device_class`, `state_class`, `find_definition`, `get_vif`, `volume_class_from_medium`; `parse_hex`. |
| [`src/record.rs`](../src/record.rs) | `Record` (Option fields + `to_json_string`). |
| [`src/header.rs`](../src/header.rs) | `Header`, `StatusDetails`. |
| [`src/frame.rs`](../src/frame.rs) | `Frame` (frame checks) and `Telegram` (frame + header + records + error). |
| [`src/error.rs`](../src/error.rs) | `MbusError` (the three codes the decoder can set) + English messages. |
| [`src/json.rs`](../src/json.rs) | Dependency-free JSON writer preserving key order. |
| [`src/tables.rs`](../src/tables.rs) | **Generated** lookup tables (see below). |
| [`tools/gen_tables.py`](../tools/gen_tables.py) | Generator that rewrites `src/tables.rs` from the Python port. |

## API notes

* `Decoder` keeps the error state like the C++/Python class. `get_error()`
  returns **and** clears the code, exactly like `MBusinoLib::getError()`.
* `Record` / `Header` / `Frame` carry `Option` fields: a field is `None` when
  the ArduinoJson original does not write that key. JSON serialisation emits
  only the present keys, so the key set matches ArduinoJson exactly.
* `decode_telegram()` is the Rust counterpart of the Python-only
  `decode_telegram()`: it validates start/stop byte, `L1 == L2`, length and
  checksum (both the plain byte sum and the quartet sum) before decoding header
  and records.

## Numerics

All numeric operations run in the same order and with the same types as the
C++ code, so the values are bit-identical:

* `int8_t` / `int16_t` / `int32_t` wrap-around: Rust's `i8` / `i16` / `i32`
  are natively the same, no emulation needed (the Python port had to emulate
  them with `_to_int8` etc.).
* `int8_t` truncation of the VIF scalar, 32-bit `float` for "real" records.
* `value_scaled` is accumulated as `f64`, with `f32` for real records.

Only ArduinoJson's *output* rounds, therefore the golden comparison uses a
tolerance for `value_scaled` (`rel_tol = 1e-6`, `abs_tol = 1e-6`,
`ARDUINO_FLOAT_DECIMALS = 6`). Everything else - `name`, `units`,
`value_string`, `vif`, `code`, `subUnit`, `storage`, `tariff`,
`telegramFollow` and the key set - must match exactly.

## Generated tables

`src/tables.rs` is generated - **do not edit it by hand**. `tools/gen_tables.py`
imports the Python modules from a local MBusinoLibPy checkout and emits the
Rust source for:

* `CODE_NAME`, `CODE_UNITS`, `DEVICE_CLASS`, `STATE_CLASS`, `MEDIUM_NAMES`
* `VIF_DEFS` (the 1:1 copy of the `vif_defs` table)

```sh
python3 tools/gen_tables.py   # rewrites src/tables.rs
```

The C++ enum order (and therefore every numeric value) is preserved because
`MBUS_CODE` is an enum whose members are numbered in declaration order.

## Testing

```sh
cargo test
```

The suite is **golden-based**: the expected outputs were produced by the
*actual C++ library* and copied verbatim from `MBusinoLibPy/tests/golden/`
into [`tests/golden/`](../tests/golden/).

* `tests/golden/*.hex` + `*.expected.json` - five real telegrams decoded by
  the original `test_decode` harness; `*.raw.txt` is its raw output,
  `*.source.json` documents where each telegram was found.
* `tests/golden/*.header.json` - `decode_header()` output for the same
  telegrams plus four negative cases.
* `tests/golden/records/*.json` - raw record buffers (the AUnit test vectors
  from `test/aunit/test.cpp`, excerpts from the golden telegrams and two
  documented edge cases).
* [`tests/golden.rs`](../tests/golden.rs) - the harness: it loads the golden
  JSON as `serde_json::Value` (dev-dependency only) and compares field by
  field with the rules above.

Result: **25 integration tests + 1 doctest green**, `cargo build` without
warnings. An additional fuzz cross-check against MBusinoLibPy (4000 record
buffers + 2944 telegrams) produced 0 mismatches.

## Differences from the C++ library

1. **No internal buffer API.** `reset()`, `getSize()`, `getBuffer()` and
   `copy()` are gone; you pass a byte slice instead.
2. **`decode_telegram()` is new** and performs the frame checks that
   `test_decode.cpp` does not (start/stop byte, `L1 == L2`, checksum).
3. **No undefined behaviour.** Wherever the C++ reads past the end of the
   buffer (several spots in `decodeRecords()`), the Rust port stops cleanly
   and sets `BufferOverflow`; the safe behaviour is inherited from the Python
   port. See MBusinoLibPy `dev_issues/open-points.md` OP-5.
4. **`get_error_message()` equivalent** is `MbusError::message()` /
   `error_message()` - readable English text per code.
5. Idiomatic Rust API instead of the C++ class internals: `Result`-free
   slicing where the original had raw pointers, `Option` instead of missing
   JSON keys, structs instead of a `JsonObject` output parameter.

## Credits

Port based on **MBusinoLib** by Zeppelin500, which is based on the
[AllWize/mbus-payload](https://github.com/allwize/mbus-payload) library with
much more decode capability. Thanks to **AllWize** for the original library,
and thanks to **HWHardsoft** and **TrystanLea** for the M-Bus communication
used by [MBusino](https://github.com/Zeppelin500/MBusino). The Python port
[MBusinoLibPy](https://github.com/Zeppelin500/MBusinoLibPy) is the blueprint
for this crate.

## License

**GPL-3.0-or-later** - same license as the C++ original. See
[`LICENSE`](../LICENSE).
