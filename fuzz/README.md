# Fuzzing twkb

[cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets, in a workspace of their own, built with nightly
and libFuzzer: `just fuzz read 60` or `just fuzz write 600` from the repository's root.

| Target | Input | What must hold |
|---|---|---|
| `read` | any bytes | Reading never panics, and stays within `ReadOptions`' memory and nesting. A geometry read whose ordinates are exact as `f64` is written again with its header's options and read back with the same structure and ordinates |
| `write` | a geometry of any type, or a collection of them, any coordinates, and options | Writing never panics; a rectangle is an error. Read back, each finite ordinate is within half a unit of the last digit kept |

The checks are in `src/lib.rs`. The `read` corpus starts from the fixtures' TWKB (`corpus/`, not in git). An input
that fails is saved in `artifacts/`; `cargo +nightly fuzz run --target <host> <target> <file>` runs it again, and
each one found so far is a test in `../tests/regressions.rs`.
