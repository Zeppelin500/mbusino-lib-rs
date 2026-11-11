# Open points - mbusino-lib-rs port

Things noticed while porting **MBusinoLib 0.10.0** to Rust. The blueprint was
the verified 1:1 Python port
[**MBusinoLibPy**](https://github.com/Zeppelin500/MBusinoLibPy), whose golden
references were produced by the *unmodified* C++ library.

Every item carries a status and a suggested fix. Fundstellen refer to the C++
original (`Zeppelin500/MBusinoLib`, local checkout `~/…/MBusinoLib`) and to
MBusinoLibPy unless stated otherwise.

The user-facing documentation lives in the [root README](../README.md); the
port documentation itself is [port-notes.md](port-notes.md).

---

## OP-1 - `publish = false` still set

* **What:** `Cargo.toml` still carries `publish = false` and version `0.1.0`.
* **Status:** open
* **Suggestion:** before the first crates.io release, flip `publish` to `true`
  (or drop the line) and finalize the metadata: `description`, `keywords`,
  `categories`, `repository`, `homepage`, `readme` and `license`
  (`GPL-3.0-or-later`). GPL-3.0-or-later is a valid crates.io license, so this
  is not a blocker - just unfinished.

## OP-2 - `decode_telegram` with a 6-byte input

* **What:** a frame shorter than 7 bytes has no CI field. The Python blueprint
  raises `IndexError` (`data[6]` out of range) here; the Rust port stays
  runnable and reports `ci_field = None`.
* **Status:** deliberate deviation - safe, and documented in `decoder.rs`.
* **Suggestion:** none. The Rust behaviour is the intended one; keep the code
  comment in sync if the frame handling changes.

## OP-3 - Empty / odd frames

* **What:** where the Python code relies on forgiving slicing, the Rust port
  uses `.get()` / slice guards instead.
* **Status:** deliberate deviation. Behaviour matches Python for every *valid*
  case (covered by the golden tests).
* **Suggestion:** none.

## OP-4 - Numerics: `f64` accumulator + `f32` for real records

* **What:** `value_scaled` is kept as `f64`, with `f32` truncation for "real"
  records - exactly like the Python blueprint. This keeps the values
  bit-identical wherever Python is bit-identical to the C++.
* **Status:** deliberate, verified. Fuzz cross-check against MBusinoLibPy:
  4000 record buffers + 2944 telegrams -> **0 mismatches**.
* **Suggestion:** none. The golden tolerance (`rel_tol = 1e-6`,
  `abs_tol = 1e-6`, `ARDUINO_FLOAT_DECIMALS = 6`) only covers ArduinoJson's
  *output* rounding, not the computation.

## OP-5 - `cargo fmt`

* **What:** `cargo fmt` was applied (rustfmt was installed in the porting
  session); no other tooling deviations.
* **Status:** done.
* **Suggestion:** keep `cargo fmt --check` + `cargo clippy` in CI once a CI
  workflow exists.

---

## Project-level

## PP-1 - Documentation integration

* **What:** `mbusino-lib-rs` is not yet linked from the MBusino / MBusinoLib
  documentation.
* **Status:** open - deliberately deferred until the owner has reviewed the
  code.
* **Suggestion:** after review, add a "Rust" entry next to the Python port in
  the MBusinoLib README (and/or the MBusino docs).

## PP-2 - The C++ open points also apply

* **What:** the C++ original has a list of unclean spots (unused error codes,
  the `snprintf` size bug, `strncpy` overflows, several out-of-bounds reads,
  the stale `address`/`a_field` key, ...) documented in
  **MBusinoLibPy `dev_issues/open-points.md`** (OP-1 … OP-14).
* **Status:** open (upstream C++; not a Rust-port issue).
* **Suggestion:** the Rust port does **not** reproduce the C++ undefined
  behaviour - it follows the Python port's bounded, clean-stop behaviour.
  Track the C++ items where the C++ lives.
