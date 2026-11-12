# twkb

[![GitHub repo](https://img.shields.io/badge/github-nyurik/twkb-8da0cb?logo=github)](https://github.com/nyurik/twkb)
[![crates.io version](https://img.shields.io/crates/v/twkb)](https://crates.io/crates/twkb)
[![crate usage](https://img.shields.io/crates/d/twkb)](https://crates.io/crates/twkb)
[![docs.rs status](https://img.shields.io/docsrs/twkb)](https://docs.rs/twkb)
[![crates.io license](https://img.shields.io/crates/l/twkb)](https://github.com/nyurik/twkb/blob/main/LICENSE-APACHE)
[![CI build status](https://github.com/nyurik/twkb/actions/workflows/ci.yml/badge.svg)](https://github.com/nyurik/twkb/actions)
[![Codecov](https://img.shields.io/codecov/c/github/nyurik/twkb)](https://app.codecov.io/gh/nyurik/twkb)

A fast reader and writer of [TWKB](https://github.com/TWKB/Specification/blob/master/twkb.md) (Tiny Well-Known
Binary), a compact geometry encoding: coordinates are rounded to a chosen number of decimal digits and stored as
varint deltas from the previous point, typically several times smaller than WKB. PostGIS writes it with
[`ST_AsTWKB`](https://postgis.net/docs/ST_AsTWKB.html) and reads it with
[`ST_GeomFromTWKB`](https://postgis.net/docs/ST_GeomFromTWKB.html).

* All geometry types and format options: z and m, sizes, bounding boxes, empty geometries, and ID lists (read only).
* The same bytes as PostGIS, except at negative precisions, where this crate rounds exactly and PostGIS does not.
* [geo-traits](https://docs.rs/geo-traits) both ways: writes any geometry that implements them (geo-types, wkt, wkb,
  and others), and the geometries it reads implement them.
* Integer coordinates as stored, without float rounding: `write_scaled_line_string` and `stored_coords`.
* Bad or hostile input is an error, never a panic or an unbounded allocation; [`ReadOptions`] caps memory and nesting.

## Usage

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

## Performance

A 1,000-point line string at OpenStreetMap's 7 digits (`just bench`, Intel i9-10885H):

| | twkb | wkb |
|---|---|---|
| size | 3,481 bytes (78% smaller) | 16,009 bytes |
| write from geo-types | 11.7 µs (596% slower) | 1.7 µs |
| read, sum the `f64` ordinates | 5.8 µs (55% slower) | 3.7 µs |


Tested against PostGIS, the test suites of other TWKB implementations, and fuzzing.

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

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual-licensed as above, without any
additional terms or conditions.
