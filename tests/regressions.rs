//! Inputs the fuzz targets found (`fuzz/`): each is read, written again with its header's options, and read back;
//! and geometries that were written wrong.
#![allow(clippy::unwrap_used, reason = "a test fails on any error")]

mod common;

use common::{hex, same, shape, to_hex};
use insta::{assert_compact_debug_snapshot, assert_snapshot};
use twkb::{Twkb, WriteOptions, write_geometry};
use wkb::reader::read_wkb;

/// Reads `twkb`, writes it again and reads it back as the same geometry; the bytes written, as hex.
fn round_trip(twkb: &str) -> String {
    let read = Twkb::new(&hex(twkb)).unwrap();
    let buf = write_again(&read);
    let again = Twkb::new(&buf).unwrap();
    assert!(
        same(&shape(&read), &shape(&again), &|_| 0.0),
        "{read:?} came back as {again:?}"
    );
    to_hex(&buf)
}

/// A geometry written with the options its header gives.
fn write_again(read: &Twkb) -> Vec<u8> {
    let header = read.header();
    let opts = WriteOptions {
        precision: header.precision(),
        size: header.size().is_some(),
        bbox: header.bbox_stored().is_some(),
        remove_repeated_points: false,
    };
    let mut buf = Vec::new();
    write_geometry(&mut buf, read, &opts).unwrap();
    buf
}

#[test]
fn collection_of_another_precision() {
    // A collection with 5 digits holding a point with 4: written again, the point takes the collection's 5
    assert_snapshot!(round_trip("a7080201810802c09a0c80b51802"), @"a7080201a1080280897a8092f40102");
}

#[test]
fn polygon_of_an_empty_ring() {
    // A multi-polygon whose polygon has one ring of no points: written again, the polygon is empty, and so the
    // multi-polygon of only it (PostGIS cannot read it, "called with n=0 and npoints=0")
    let read = Twkb::new(&hex("0608fa010100")).unwrap();
    assert_snapshot!(to_hex(&write_again(&read)), @"0618fa");
}

#[test]
fn collection_box_of_fewer_dimensions() {
    // A collection of 4 dimensions with a box, holding geometries of 2: its box spans what they have
    assert_snapshot!(round_trip(concat!(
        "0709030014001402080408030303",
        "1b000400040205000004000004030000030500000002020000010100",
        "021803",
        "01090306000800",
        "0a000c00",
        "06080a0c"
    )), @"070903000600080a000c00030301000400040205000004000004030000030500000002020000010100021803010903060008000a000c0006080a0c");
}

#[test]
fn multi_point_with_an_empty_point() {
    // MULTIPOINT(EMPTY,(1 2)) as WKB, its empty point of NaN: the empty point is left out, as PostGIS does
    let wkb = hex(
        "0104000000020000000101000000000000000000f87f000000000000f87f0101000000000000000000f03f0000000000000040",
    );
    let mut buf = Vec::new();
    write_geometry(&mut buf, &read_wkb(&wkb).unwrap(), &WriteOptions::default()).unwrap();
    assert_snapshot!(to_hex(&buf), @"0400010204");
}

#[test]
fn collection_too_fine_for_its_geometries() {
    // A collection with 5 digits holding a point of a large x with fewer: written with the collection's 5, the x is
    // beyond 64 bits, an error
    let read = Twkb::new(&hex("a70818028618cf02040280ffffffffffff0006ff0404")).unwrap();
    let mut buf = Vec::new();
    let opts = WriteOptions {
        precision: read.header().precision(),
        ..WriteOptions::default()
    };
    assert_compact_debug_snapshot!(write_geometry(&mut buf, &read, &opts), @"Err(OutOfRange { value: 281474976710592.0, digits: 5 })");
}

#[test]
fn failed_write_leaves_the_buffer() {
    use geo_types::{Geometry, GeometryCollection, Point, Rect};
    let point = |x: f64| Geometry::Point(Point::new(x, 1.0));
    let failed = [
        point(f64::NAN),
        Geometry::GeometryCollection(GeometryCollection::new_from(vec![
            point(1.0),
            point(f64::INFINITY),
        ])),
        Geometry::GeometryCollection(GeometryCollection::new_from(vec![
            point(1.0),
            Geometry::Rect(Rect::new((0.0, 0.0), (1.0, 1.0))),
        ])),
    ]
    .map(|g| {
        let mut buf = vec![0xaa, 0xbb];
        let result = write_geometry(&mut buf, &g, &WriteOptions::default());
        format!("{result:?} {}", to_hex(&buf))
    });
    assert_snapshot!(failed.join("\n"), @r#"
    Err(OutOfRange { value: NaN, digits: 0 }) aabb
    Err(OutOfRange { value: inf, digits: 0 }) aabb
    Err(Unsupported("rectangle")) aabb
    "#);
}
