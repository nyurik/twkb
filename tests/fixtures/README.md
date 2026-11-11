# Fixtures

TWKB test cases from other implementations, one module per source with its license, the commit it came from and a
README linking the files the cases were taken from, and this crate's own cases in `own`. `main.rs` makes them one test
target, `fixtures`.

Each case is a `case!` with its expected values as [insta](https://insta.rs) inline snapshots, in a function that
takes a `Runner` as `r`:

```rust,ignore
// WKT written with x and y precision, z and m precision, the size and the box: the TWKB, then what it reads as
case!(r, "POINT (1 1)", 0, 0, 0, false, true, @"
0101020002000202
POINT(1 1)
");
// TWKB only read: what it reads as, with the IDs of a multi-geometry's parts
case!(r, "04070b0004020402000200020404", @"MULTIPOINT((0 1),(2 3)) ids [0, 1]");
```

Every geometry read is also written again with the options its header gives, and must give the bytes it was read
from. `; ` separates the geometries of a stream. The TWKB hex of each case is the source's own; the WKT is what this
crate reads it as, which matched the source's expected geometry when the cases were taken. The TWKB of `jts`,
`postgis` and `own` is PostGIS's, which leaves out repeated points, so their cases are written with
`remove_repeated_points`.

## Against PostGIS

A module's `cases!` lists its case functions: each has a `#[test]` that checks its snapshots, and `against_postgis.rs`
collects all of them as data and checks them against PostGIS in one batch, with 20,000 random geometries from a fixed
seed, of longitudes and latitudes and of values up to 1e11 (below 0 digits, up to 1e5 once scaled). It is ignored by
default: `just test-postgis` starts PostGIS in Docker and runs it (`just stop-postgis` stops it), and
`just test-postgis-at <url>` runs it against another PostGIS. PostGIS must write each case's WKT as its TWKB, read
each TWKB as the geometry this crate reads, and write each random geometry as this crate does. The differences it
has on purpose are in `KNOWN`, each with its reason: below 0 digits PostGIS writes as if scaled by a single-precision
10^precision, so it rounds values near a half otherwise, which the random geometries leave out; and it reads divided
by an `f64` 10^-precision, so what it reads there is compared within that rounding, 3.4e-16 of each value.

To change an expected value, run `cargo insta test --review`. The directory is not in the published crate (`include`
in `Cargo.toml`): some of it is under licenses the crate is not.
