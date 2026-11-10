# Golden references (expected output produced by the C++ original)

Everything in this folder was produced by the **unmodified C++ library
`MBusinoLib`** and serves as the reference output for the Python tests.
Nothing here was invented or calculated by hand.

## Layout

| File | Content |
|------|---------|
| `<name>.hex` | Input: one complete telegram, space separated like `./test_decode "68 56 ..."` |
| `<name>.expected.json` | Expected result: `slave_address`, `records_decoded`, `error_code`, `records` (the JSON array parsed from the harness output) |
| `<name>.raw.txt` | Raw harness output (unchanged, including the `[n] name (vif) = value` lines; line endings normalised to LF) |
| `<name>.source.json` | **Provenance**: source file + line number, recognition features, special remarks |
| `<name>.header.json` | Expected result of `decodeHeader()` plus `header_ok` (the boolean C++ return value) |
| `records/<name>.json` | Raw record buffers without a telegram frame: `hex`, `records_decoded`, `error_code`, `records`, `source` |

## The five telegrams

All of them come from the repositories searched for this port
(`MBusino` and `MBusinoLib`); file + line is stored in each `source.json`:

| Name | Bytes | L | Records | Checksum | Manufacturer / ID / Medium | Where it was found |
|------|-------|---|---------|----------|----------------------------|--------------------|
| `efe_heat_meter` | 199 | 193 | 29 | 0x75 valid | EFE / 73753409 / Heat (Mass) | `MBusino/archive/telegram_test/telegram_test.ino:94` (active `mbus_data[]`) |
| `ernie_power_meter` | 164 | 158 | 16 | 0x8B valid | IME / 00067609 / Electricity | `MBusino/playground/MBusino_ernie/MBusino_ernie.ino:514` |
| `hw_thermostat` | 89 | 83 | 12 | 0x9F valid | ELV / 24008211 / Unknown | `MBusino/archive/telegram_test/telegram_test.ino:92` |
| `mbusino_example` | 202 | 196 | 30 | 0xFB valid | WZG / 29079172 / Cold water | `MBusinoLib/examples/MbusinoLibExample/MbusinoLibExample.ino:15` |
| `elster` | 37 | 31 | 2 | **0xCC instead of 0xCF - invalid** | ELS / 12345678 / Gas | `MBusino/archive/telegram_test/telegram_test.ino:96` |

Paths covered:

* `efe_heat_meter` - the longest realistic set of records: 32-bit integer,
  BCD, real, date type F and type G, double volume VIF (0x933A/0x943A),
  storage/tariff DIFEs.
* `ernie_power_meter` - three-phase electricity meter with phase VIFEs
  (0xFC/0xFF → `_L1`…), subUnit, unscaled 0xFF manufacturer data and the
  **0x0F marker** ("manufacturer specific" abort, `MBusinoLib.cpp` line 648).
* `hw_thermostat` - ASCII VIF (0xFC with its own unit text) and the
  **0x1F marker** ("more records follow", `telegramFollow`, line 652).
* `mbusino_example` - 30 records, storage 1–6, status/error records, date type G.
* `elster` - **deliberately broken checksum** (typo in the source). The C++
  original does not check checksums and decodes it anyway; the test verifies
  that `decode_telegram()` makes exactly that visible as
  `frame["ok"] = False` while the record count stays the same.

More details for every telegram (why it was chosen and which edge case it
covers) are in the respective `*.source.json`.

## Record goldens (`records/`)

* `aunit_*` - the eight decoder test vectors from
  `MBusinoLib/test/aunit/test.cpp` (`Number_1`…`Number_6`, `MultiField`,
  `Decode_2bcd`, `Decode_8bcd`). Careful: that file is outdated for the
  current library (it calls encoder methods that no longer exist and
  `Find_Definition` expects index 0 although `UNKNOWN_VIF` now sits at the
  front of the `vif_defs` table), therefore only the *buffers* are used, not
  the expectations written there.
* `excerpt_*` - real record excerpts from the golden telegrams (variable
  length ASCII from `elster`, date type F/G from `efe_heat_meter`, BCD with
  the F half-byte negative sign from `hw_thermostat`).
* `konstrukt_*` - two deliberately constructed edge cases covering DIF 0x03
  (24 bit) and DIF 0x0D + LVAR 0xC0 (positive BCD). Both are single-buffer
  unit tests, **not** telegrams and not part of the telegram golden set; the
  origin is stored in each file's `source` field.

`records/…/konstrukt_var_bcd` is especially interesting: the C++ harness
reports `Records decoded: 0` (the return value on `BUFFER_OVERFLOW`) while
the JSON array already contains one record. The Python test checks exactly
that behaviour.

## How these files were produced

See [`tools/README.md`](../../tools/README.md) - in short:

1. `test_decode` (pre-built in the original repo) for `.hex`/`.expected.json`/`.raw.txt`
2. `tools/test_decode_header.cpp` for `.header.json`
3. `tools/test_decode_records.cpp` for `records/*.json`

For `mbusino_example` a wrapper with a larger JSON buffer was needed
(4096 bytes are not enough for 30 records; the wrapper is otherwise
byte-identical, which is proven on the four shorter telegrams).

## Comparison rules used by the tests

* `name`, `units`, `value_string`, `vif`, `code`, `subUnit`, `storage`,
  `tariff`, `telegramFollow` **exactly**, and so does the key set.
* `value_scaled` with `math.isclose(rel_tol=1e-6, abs_tol=1e-6)`
  (`ARDUINO_FLOAT_DECIMALS = 6`), because ArduinoJson rounds while
  serialising: small values with 6 decimals, large ones with 9 significant
  digits (`202602132030` → `"2.02602132e11"`).
  Measured over all 123 golden records: 89 of 102 numeric values are
  bit-identical, the largest deviation is `7.3e-08` relative (ArduinoJson
  rounds one date value), zero key mismatches.
* Record count and error code must match exactly.
