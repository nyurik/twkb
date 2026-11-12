//! What the cases of `compat/` do not reach: the accessors, every error's message, and the errors and edge cases of
//! writing.
#![allow(clippy::unwrap_used, reason = "a test fails on any error")]

mod common;

use std::str::FromStr as _;
use std::vec;

use common::{hex, shape, to_hex};
use geo_traits::{
    Dimensions, GeometryCollectionTrait as _, GeometryTrait, GeometryType as Kind,
    LineStringTrait as _, MultiLineStringTrait as _, MultiPointTrait as _, MultiPolygonTrait as _,
    PointTrait, PolygonTrait as _, UnimplementedCoord, UnimplementedGeometryCollection,
    UnimplementedLine, UnimplementedLineString, UnimplementedMultiLineString,
    UnimplementedMultiPoint, UnimplementedMultiPolygon, UnimplementedPolygon, UnimplementedRect,
    UnimplementedTriangle,
};
use insta::{assert_compact_debug_snapshot, assert_snapshot};
use twkb::{Precision, Twkb, TwkbError, WriteOptions, write_geometry, write_scaled_line_string};

/// `wkt` written with a bounding box and 1 digit, and read back.
fn read(wkt: &str) -> Twkb {
    let opts = WriteOptions {
        precision: Precision { xy: 1, z: 1, m: 1 },
        bbox: true,
        ..WriteOptions::default()
    };
    let mut buf = Vec::new();
    write_geometry(&mut buf, &wkt::Wkt::<f64>::from_str(wkt).unwrap(), &opts).unwrap();
    Twkb::new(&buf).unwrap()
}

/// The error of writing `geom` with `opts`.
fn write_error(geom: &impl GeometryTrait<T = f64>, opts: WriteOptions) -> TwkbError {
    write_geometry(&mut Vec::new(), geom, &opts).unwrap_err()
}

#[test]
fn header() {
    let g = read("LINESTRING Z(1 2 3,4.5 -6 7)");
    let h = g.header();
    assert_compact_debug_snapshot!(h.bbox().unwrap().collect::<Vec<_>>(), @"[(1.0, 4.5), (-6.0, 2.0), (3.0, 7.0)]");
    assert_compact_debug_snapshot!((h.is_empty(), read("POINT EMPTY").header().is_empty()), @"(false, true)");
    assert_compact_debug_snapshot!(g.stored_coords(), @"[10, 20, 30, 45, -60, 70]");
    let Twkb::LineString(l) = &g else {
        unreachable!()
    };
    assert_compact_debug_snapshot!(l.coord(1).unwrap().stored(), @"[45, -60, 70]");
    let c = read("GEOMETRYCOLLECTION(POINT(1 2))");
    assert_compact_debug_snapshot!(c.stored_coords(), @"[]");
}

#[test]
fn each_type() {
    // Each type's own header, stored ordinates and geo-traits, and those of the parts it lends
    let typed = |g: &Twkb| match g {
        Twkb::Point(g) => format!(
            "{:?} {:?} {:?}",
            g.header().dimension(),
            g.stored_coords(),
            shape(g)
        ),
        Twkb::LineString(g) => format!("{:?} {:?}", g.stored_coords(), shape(g)),
        Twkb::Polygon(g) => {
            let ring = g.exterior().unwrap();
            format!("{:?} {:?} {:?}", g.stored_coords(), shape(g), shape(&ring))
        }
        Twkb::MultiPoint(g) => {
            let p = g.point(1).unwrap();
            format!(
                "{:?} {:?} {:?}",
                g.stored_coords(),
                shape(g),
                (p.dim(), shape(&p))
            )
        }
        Twkb::MultiLineString(g) => {
            format!(
                "{:?} {:?} {:?}",
                g.stored_coords(),
                shape(g),
                shape(&g.line_string(0).unwrap())
            )
        }
        Twkb::MultiPolygon(g) => {
            format!(
                "{:?} {:?} {:?}",
                g.stored_coords(),
                shape(g),
                shape(&g.polygon(0).unwrap())
            )
        }
        Twkb::GeometryCollection(g) => format!(
            "{:?} {:?} {:?}",
            g.header().geometry_type(),
            shape(g),
            g.geometry(0).map(GeometryTrait::dim)
        ),
    };
    let all = [
        "POINT M(1 2 3)",
        "LINESTRING(1 2,3 4)",
        "POLYGON((0 0,1 0,1 1,0 0))",
        "MULTIPOINT(1 2,3 4)",
        "MULTILINESTRING((1 2,3 4))",
        "MULTIPOLYGON(((0 0,1 0,1 1,0 0)))",
        "GEOMETRYCOLLECTION(POINT(1 2))",
    ]
    .map(|wkt| {
        let g = read(wkt);
        assert_eq!(GeometryTrait::dim(&g), GeometryTrait::dim(&&g), "{wkt}");
        format!("{:?} {}", g.stored_coords(), typed(&g))
    });
    assert_snapshot!(all.join("\n"), @"
    [10, 20, 30] Xym [10, 20, 30] Point([1.0, 2.0, 3.0])
    [10, 20, 30, 40] [10, 20, 30, 40] LineString([[1.0, 2.0], [3.0, 4.0]])
    [0, 0, 10, 0, 10, 10, 0, 0] [0, 0, 10, 0, 10, 10, 0, 0] Polygon([[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]]]) LineString([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]])
    [10, 20, 30, 40] [10, 20, 30, 40] MultiPoint([[1.0, 2.0], [3.0, 4.0]]) (Xy, Point([3.0, 4.0]))
    [10, 20, 30, 40] [10, 20, 30, 40] MultiLineString([[[1.0, 2.0], [3.0, 4.0]]]) LineString([[1.0, 2.0], [3.0, 4.0]])
    [0, 0, 10, 0, 10, 10, 0, 0] [0, 0, 10, 0, 10, 10, 0, 0] MultiPolygon([[[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]]]]) Polygon([[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]]])
    [] GeometryCollection GeometryCollection([Point([1.0, 2.0])]) Some(Xy)
    ");
}

#[test]
fn errors() {
    let all = [
        TwkbError::UnexpectedEnd(3),
        TwkbError::VarintTooLong(3),
        TwkbError::UnknownType(9),
        TwkbError::CountTooLarge { count: 9, at: 3 },
        TwkbError::SizeMismatch {
            expected: 9,
            actual: 3,
        },
        TwkbError::TrailingBytes(3),
        TwkbError::TooDeep(3),
        TwkbError::MemoryLimit(3),
        TwkbError::Precision(9),
        TwkbError::Dimensions(5),
        TwkbError::Unsupported("triangle"),
        TwkbError::OutOfRange {
            value: f64::INFINITY,
            digits: 7,
        },
        TwkbError::LengthMismatch { len: 3, items: 2 },
    ]
    .map(|e| e.to_string());
    assert_snapshot!(all.join("\n"), @"
    the TWKB ends inside a geometry, at byte 3
    the varint at byte 3 does not fit 64 bits
    unknown TWKB geometry type 9
    a count of 9 at byte 3 is more than the rest of the TWKB can hold
    the TWKB's size field says 9 bytes, but its body has 3
    3 bytes follow the TWKB geometry
    geometry collections are nested deeper than 3
    decoding the TWKB would take more than 3 bytes of memory
    a precision of 9 decimal digits is out of range: x and y take -8 to 7, z and m 0 to 7
    5 dimensions cannot be written as TWKB, which has 2 to 4
    a triangle cannot be written as TWKB: write it as a polygon
    the ordinate inf cannot be stored with 7 decimal digits in 64 bits
    an iterator of length 3 yielded 2 items
    ");
}

#[test]
fn bad_varint_in_a_line() {
    // A line string of x and y, its x and then its y a varint of more than 64 bits
    let ten = "ffffffffffffffffff7f";
    assert_compact_debug_snapshot!(Twkb::new(&hex(&format!("020001{ten}00"))), @"Err(VarintTooLong(3))");
    assert_compact_debug_snapshot!(Twkb::new(&hex(&format!("02000100{ten}"))), @"Err(VarintTooLong(4))");
}

#[test]
fn precision_out_of_range() {
    let point = wkt::Wkt::<f64>::from_str("POINT Z(1 2 3)").unwrap();
    let opts = |z| WriteOptions {
        precision: Precision { xy: 0, z, m: 0 },
        ..WriteOptions::default()
    };
    assert_compact_debug_snapshot!(write_error(&point, opts(8)), @"Precision(8)");
}

/// A point of 5 dimensions, which TWKB cannot hold.
struct FiveDimensions;

impl PointTrait for FiveDimensions {
    type CoordType<'a> = UnimplementedCoord<f64>;

    fn coord(&self) -> Option<UnimplementedCoord<f64>> {
        None
    }
}

impl GeometryTrait for FiveDimensions {
    type T = f64;
    type PointType<'b> = Self;
    type LineStringType<'b> = UnimplementedLineString<f64>;
    type PolygonType<'b> = UnimplementedPolygon<f64>;
    type MultiPointType<'b> = UnimplementedMultiPoint<f64>;
    type MultiLineStringType<'b> = UnimplementedMultiLineString<f64>;
    type MultiPolygonType<'b> = UnimplementedMultiPolygon<f64>;
    type GeometryCollectionType<'b> = UnimplementedGeometryCollection<f64>;
    type RectType<'b> = UnimplementedRect<f64>;
    type TriangleType<'b> = UnimplementedTriangle<f64>;
    type LineType<'b> = UnimplementedLine<f64>;

    fn dim(&self) -> Dimensions {
        Dimensions::Unknown(5)
    }

    fn as_type(
        &self,
    ) -> Kind<
        '_,
        Self,
        UnimplementedLineString<f64>,
        UnimplementedPolygon<f64>,
        UnimplementedMultiPoint<f64>,
        UnimplementedMultiLineString<f64>,
        UnimplementedMultiPolygon<f64>,
        UnimplementedGeometryCollection<f64>,
        UnimplementedRect<f64>,
        UnimplementedTriangle<f64>,
        UnimplementedLine<f64>,
    > {
        Kind::Point(self)
    }
}

#[test]
fn dimensions_out_of_range() {
    assert_compact_debug_snapshot!(write_error(&FiveDimensions, WriteOptions::default()), @"Dimensions(5)");
}

#[test]
fn lines_and_triangles() {
    use geo_types::{Line, Triangle};
    // A line is written as a line string, with or without a bounding box; a triangle is refused
    let line = Line::new((1.0, 2.0), (3.0, 4.0));
    let written = [false, true].map(|bbox| {
        let mut buf = Vec::new();
        let opts = WriteOptions {
            bbox,
            ..WriteOptions::default()
        };
        write_geometry(&mut buf, &line, &opts).unwrap();
        to_hex(&buf)
    });
    assert_compact_debug_snapshot!(written, @r#"["02000202040404", "0201020404040202040404"]"#);
    let triangle = Triangle::new((0.0, 0.0).into(), (1.0, 0.0).into(), (1.0, 1.0).into());
    assert_compact_debug_snapshot!(write_error(&triangle, WriteOptions::default()), @r#"Unsupported("triangle")"#);
}

#[test]
fn bbox_of_no_coordinates() {
    // A polygon of two rings of no points is not empty, but has no coordinate: written with a box, it is 0 to 0
    let read = Twkb::new(&hex("0300020000")).unwrap();
    let mut buf = Vec::new();
    let opts = WriteOptions {
        bbox: true,
        ..WriteOptions::default()
    };
    write_geometry(&mut buf, &read, &opts).unwrap();
    assert_snapshot!(to_hex(&buf), @"030100000000020000");
}

/// An iterator that says it has 3 points, whatever it yields.
struct Says3(vec::IntoIter<[i64; 2]>);

impl Iterator for Says3 {
    type Item = [i64; 2];

    fn next(&mut self) -> Option<[i64; 2]> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (3, Some(3))
    }
}

impl ExactSizeIterator for Says3 {}

#[test]
fn line_string_of_another_length() {
    let mut buf = vec![0xaa];
    let result = write_scaled_line_string(&mut buf, 0, Says3(vec![[1, 2], [3, 4]].into_iter()));
    assert_compact_debug_snapshot!((result, to_hex(&buf)), @r#"(Err(LengthMismatch { len: 3, items: 2 }), "aa")"#);
}
