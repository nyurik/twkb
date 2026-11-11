# Working on twkb

A reader and writer of TWKB, Tiny Well-Known Binary. These notes are for anyone changing the code, people and coding
agents alike.

## Layout

| Path | Holds |
|---|---|
| `src/common.rs` | The header's fields, precision, rounding, varints and zigzag |
| `src/reader/` | `Twkb::new`, `ReadOptions`, the cursor and its memory budget, the geometries and their geo-traits |
| `src/writer.rs` | `write_geometry` for any geo-traits geometry, and the writers of integer coordinates |
| `tests/` | Round trips of the wkb crate's samples (`wkb/`), and the inputs the fuzzers found (`regressions.rs`) |
| `compat/` | A crate of other implementations' cases, each with README and license, our own, and the PostGIS check |
| `fuzz/` | The cargo-fuzz targets, in a workspace of their own (`just fuzz`) |
| `benches/`, `examples/` | The criterion bench against wkb; conversions between TWKB and WKT or WKB |

## Design

- Performance first, while exactly correct: what PostGIS reads back from what is written, and the same bytes it
  writes, but for the differences `KNOWN` in `compat/against_postgis.rs` lists with their reasons. Measure
  with `just bench` before and after a change to a hot path.
- The interface is geo-traits, as in the wkb crate, so the code could move there: no conversions to or from WKB, WKT
  or geo-types outside the tests, examples and benches, which use them as dev-dependencies.
- Bad input is an error, never a panic or an unbounded allocation: check counts against the bytes left, and charge
  `ReadOptions`' budget before allocating.
- Errors are `TwkbError`; public signatures name it, with no `Result` alias.
- Unsafe code is denied. Only the impls of geo-traits' `unsafe fn` accessors opt in, with bounds-checked bodies.

## Licenses of other projects

The crate is MIT OR Apache-2.0. Code under the GPL, LGPL or AGPL is never read, copied or adapted. Test data, inputs
and their expected outputs, may come from any project, as `case!`s with insta inline snapshots, with its license beside
them in `compat/<source>/` and a README linking the files they were taken from at their commit; PostGIS's GPL
regression cases are there this way. `compat/` is a crate of its own, `twkb-compat`, never published, and so outside
the published package; its own code (`main.rs`, `against_postgis.rs`, `own/`) is under the crate's licenses.

## Writing

- Docs and comments describe the code as it is: no plans, roadmaps or history.
- Public items carry docs that say more than their names: units, ranges, when an error happens.
- Performance claims come from measurements with the repository's own tools, and say where they were taken.

## Checks

`just ci-test` runs what CI runs: formatting, clippy, tests, docs and the examples. `just test-postgis` checks every
case against PostGIS in Docker, as CI does in a job of its own. After changing the reader or the writer, run
`just fuzz read 300` and `just fuzz write 300`, and add any input they find to `tests/regressions.rs`. A new case goes
into its source's module, or into `own` when no source has it, with the snapshot `cargo insta test --review` gives
after checking it against PostGIS.
