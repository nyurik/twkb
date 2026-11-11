# twkb

[![GitHub repo](https://img.shields.io/badge/github-nyurik/twkb-8da0cb?logo=github)](https://github.com/nyurik/twkb)
[![crates.io version](https://img.shields.io/crates/v/twkb)](https://crates.io/crates/twkb)
[![crate usage](https://img.shields.io/crates/d/twkb)](https://crates.io/crates/twkb)
[![docs.rs status](https://img.shields.io/docsrs/twkb)](https://docs.rs/twkb)
[![crates.io license](https://img.shields.io/crates/l/twkb)](https://github.com/nyurik/twkb/blob/main/LICENSE-APACHE)
[![CI build status](https://github.com/nyurik/twkb/actions/workflows/ci.yml/badge.svg)](https://github.com/nyurik/twkb/actions)
[![Codecov](https://img.shields.io/codecov/c/github/nyurik/twkb)](https://app.codecov.io/gh/nyurik/twkb)

A fast reader and writer of [TWKB](https://github.com/TWKB/Specification/blob/master/twkb.md), Tiny Well-Known
Binary: geometries with each coordinate as varints of its steps from the one before, rounded to a chosen number of
decimal digits. PostGIS writes it with [`ST_AsTWKB`](https://postgis.net/docs/ST_AsTWKB.html) and reads it with
[`ST_GeomFromTWKB`](https://postgis.net/docs/ST_GeomFromTWKB.html).

* Every geometry type and option of the format: points, line strings, polygons, their multi-geometries and
  collections; z and m; sizes, bounding boxes and empty geometries, read and written, and ID lists, read.
* The same bytes as PostGIS's `ST_AsTWKB`, checked against it for every test case and 20,000 random geometries, but
  below 0 digits: PostGIS writes x and y there as if scaled by a single-precision 10^precision, and so rounds wrongly
  within about 5e-8 of the scaled value from a half (150 at -2 digits is 100, 3570264.95 at -1 is 3570270), where
  this crate scales by the exact power of 10 and rounds half away from zero.
* [geo-traits](https://docs.rs/geo-traits) both ways: the writer takes any geometry that implements them (geo-types,
  wkt, wkb and others), and what the reader returns implements them, so `geo_traits::to_geo` turns it into a
  `geo_types::Geometry`.
* Integer coordinates as they are stored, for sources that have them: `write_scaled_line_string` and
  `stored_coords` skip rounding and floats altogether.
* Bad or hostile input is an error, never a panic or an unbounded allocation: every count is checked against the
  bytes left, and [`ReadOptions`] caps memory and nesting.

```rust
use geo_traits::{CoordTrait as _, LineStringTrait as _};
use twkb::{write_geometry, Precision, Twkb, WriteOptions};

fn write_and_read() -> Result<(), twkb::TwkbError> {
    let line = geo_types::LineString::from(vec![
        (-71.036_671_8, 42.373_449_3),
        (-71.036_761_9, 42.373_312),
    ]);
    let mut buf = Vec::new();
    let opts = WriteOptions {
        precision: Precision::xy(7),
        ..WriteOptions::default()
    };
    write_geometry(&mut buf, &line, &opts)?;

    let Twkb::LineString(read) = Twkb::new(&buf)? else {
        unreachable!()
    };
    assert_eq!(
        read.coord(1).map(|c| c.x_y()),
        Some((-71.036_761_9, 42.373_312))
    );
    assert_eq!(
        read.stored_coords(),
        [-710_366_718, 423_734_493, -710_367_619, 423_733_120]
    );
    Ok(())
}
```

## Precision

A coordinate is stored as `round(value * 10^digits)`, half away from zero: x and y take -8 to 7 digits (a negative
precision rounds to tens, hundreds and so on; PostGIS reads -8 but writes -7 at the least), z and m 0 to 7. Read back,
a value is within half a unit of its last digit of what was written, and exactly what was written when it had no more
digits than that, as OpenStreetMap's 7 do. An ordinate that is not finite, or beyond an `i64` once scaled, is an
error. A geometry collection's precision applies to all its geometries.

## Examples

`examples/` converts between TWKB and WKT or WKB, through geo-traits and the [wkt](https://crates.io/crates/wkt) and
[wkb](https://crates.io/crates/wkb) crates:

```shell
# WKT or hex WKB to hex TWKB, with 7 decimal digits for x and y
cargo run --example encode -- 'LINESTRING(-71.0366718 42.3734493,-71.0367619 42.373312)' --precision 7
cargo run --example encode -- 0101000000000000000000f03f0000000000000040
# with z, its own precision, the size and the bounding box
cargo run --example encode -- 'POINT Z (1.25 2.5 3)' --precision 1,0 --size --bbox

# hex TWKB to WKT and hex WKB, with what its header holds
cargo run --example decode -- e20002fbd7baa505bab38d9403890eb915
cargo run --example decode -- 04070b0004020402000200020404
```

## Tests

[`compat/`](https://github.com/nyurik/twkb/tree/main/compat) holds the test cases of other implementations, each
with its license and the commit it came from: JTS's test data, which PostGIS generated, PostGIS's own regression
tests, and the tests of wkx, twkb.js, twkbpy, twkbC and a Go reader, each with its expected TWKB and the geometry it
reads as in [insta](https://insta.rs) inline snapshots, beside cases of our own. Every case is decoded, written again
and compared byte for byte, and written from its source WKT where it has one. `just test-postgis` checks every
case, and 20,000 random geometries, against PostGIS in Docker in one batch: the same bytes written, and the same
geometries read. The wkb crate's sample geometries are written at several precisions and read back, and passed
through the wkb crate. [`fuzz/`](https://github.com/nyurik/twkb/tree/main/fuzz) holds the cargo-fuzz targets
(`just fuzz read 60`), and `tests/regressions.rs` every input they found.

`compat/` is a crate of its own in the repository, out of the published one: some of the cases are under licenses
the crate is not, PostGIS's GPL among them; they are test data only, taken as such, and no code of those projects.

## Performance

`just bench` writes and reads a line string of 1,000 points as OpenStreetMap has them (7 digits, steps of a few
metres), and the same as WKB with the wkb crate. On an Intel i9-10885H laptop (2026-11-11):

| | twkb | wkb |
|---|---|---|
| size | 3,481 bytes | 16,009 bytes |
| write integers (`write_scaled_line_string`) | 5.1 µs | |
| write from geo-types | 13.4 µs | 1.9 µs |
| read, sum the stored integers | 4.9 µs | |
| read, sum the `f64` ordinates | 12.7 µs | 3.8 µs |

TWKB's steps cost decoding that WKB's plain `f64`s do not; it is a quarter of the size.

## Development

* This project is easier to develop with [just](https://github.com/casey/just#readme), a modern alternative to `make`.
  Install it with `cargo install just`.
* To get a list of available commands, run `just`.
* To run tests, use `just test`.

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)
  at your option.

The test cases in `compat/`'s source directories are under their own projects' licenses, each beside them.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual-licensed as above, without any
additional terms or conditions.
