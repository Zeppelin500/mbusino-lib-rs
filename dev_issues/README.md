# dev_issues - internal development notes

This folder holds the **internal development notes** of the `mbusino-lib-rs`
port. They are written for the people working on the port - full of technical
detail (module map, C++/Python references, test internals, known deviations) -
and are **not meant for end users**. The notes are in English, like the rest
of the repository.

**User documentation lives in the [root README](../README.md).**

## Contents

| File | What it is |
|------|------------|
| [port-notes.md](port-notes.md) | The detailed port documentation: architecture, module map, differences from the C++ library and the Python blueprint, table generation, test/golden internals, credits. |
| [open-points.md](open-points.md) | Open points and deliberate deviations noticed while porting - each with fundstelle, status and a suggested fix. |

If you are looking for installation instructions, the API reference or the
JSON output, use the [root README](../README.md) instead.
