//! What the tests share: a geometry as a tree of its ordinates, for comparing any two geometries that implement
//! geo-traits.
#![allow(dead_code, reason = "each test file uses part of it")]

use geo_traits::{
    CoordTrait, GeometryCollectionTrait as _, GeometryTrait, GeometryType as Kind, LineStringTrait,
    LineTrait as _, MultiLineStringTrait as _, MultiPointTrait as _, MultiPolygonTrait as _,
    PointTrait as _, PolygonTrait,
};

/// A geometry's type and ordinates.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Point(Vec<f64>),
    LineString(Vec<Vec<f64>>),
    Polygon(Vec<Vec<Vec<f64>>>),
    MultiPoint(Vec<Vec<f64>>),
    MultiLineString(Vec<Vec<Vec<f64>>>),
    MultiPolygon(Vec<Vec<Vec<Vec<f64>>>>),
    GeometryCollection(Vec<Self>),
}

fn coord(c: &impl CoordTrait<T = f64>) -> Vec<f64> {
    (0..c.dim().size()).map(|n| c.nth_or_panic(n)).collect()
}

fn line(l: &impl LineStringTrait<T = f64>) -> Vec<Vec<f64>> {
    l.coords().map(|c| coord(&c)).collect()
}

fn polygon(p: &impl PolygonTrait<T = f64>) -> Vec<Vec<Vec<f64>>> {
    p.exterior()
        .into_iter()
        .chain(p.interiors())
        .map(|r| line(&r))
        .filter(|r| !r.is_empty())
        .collect()
}

/// The shape of any geometry.
pub fn shape(g: &impl GeometryTrait<T = f64>) -> Shape {
    match g.as_type() {
        Kind::Point(p) => Shape::Point(p.coord().map(|c| coord(&c)).unwrap_or_default()),
        Kind::LineString(l) => Shape::LineString(line(l)),
        Kind::Line(l) => Shape::LineString(vec![coord(&l.start()), coord(&l.end())]),
        Kind::Polygon(p) => Shape::Polygon(polygon(p)),
        Kind::MultiPoint(m) => Shape::MultiPoint(
            m.points()
                .filter_map(|p| p.coord().map(|c| coord(&c)))
                .collect(),
        ),
        Kind::MultiLineString(m) => {
            Shape::MultiLineString(m.line_strings().map(|l| line(&l)).collect())
        }
        Kind::MultiPolygon(m) => Shape::MultiPolygon(m.polygons().map(|p| polygon(&p)).collect()),
        Kind::GeometryCollection(c) => {
            Shape::GeometryCollection(c.geometries().map(|g| shape(&g)).collect())
        }
        Kind::Rect(_) | Kind::Triangle(_) => unreachable!("not in the cases"),
    }
}

/// Whether two shapes are the same, their ordinates within `tolerance(ordinate)` of each other.
pub fn same(a: &Shape, b: &Shape, tolerance: &dyn Fn(f64) -> f64) -> bool {
    let ord = |a: &[f64], b: &[f64]| {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tolerance(*x))
    };
    let line = |a: &[Vec<f64>], b: &[Vec<f64>]| {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| ord(x, y))
    };
    let poly = |a: &[Vec<Vec<f64>>], b: &[Vec<Vec<f64>>]| {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| line(x, y))
    };
    match (a, b) {
        (Shape::Point(a), Shape::Point(b)) => ord(a, b),
        (Shape::LineString(a), Shape::LineString(b))
        | (Shape::MultiPoint(a), Shape::MultiPoint(b)) => line(a, b),
        (Shape::Polygon(a), Shape::Polygon(b))
        | (Shape::MultiLineString(a), Shape::MultiLineString(b)) => poly(a, b),
        (Shape::MultiPolygon(a), Shape::MultiPolygon(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| poly(x, y))
        }
        (Shape::GeometryCollection(a), Shape::GeometryCollection(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same(x, y, tolerance))
        }
        _ => false,
    }
}

/// The relative difference two printed decimals may have.
pub fn printed(v: f64) -> f64 {
    1e-12 * v.abs().max(1.0)
}

pub fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

pub fn to_hex(b: &[u8]) -> String {
    b.iter().fold(String::new(), |mut s, b| {
        use std::fmt::Write as _;
        write!(s, "{b:02x}").unwrap();
        s
    })
}

/// `g` as WKT: numbers as Rust prints an `f64` (the shortest that reads back the same), `EMPTY` for empty geometries
/// and parts, and `Z`, `M` or `ZM` after the type for more than x and y.
///
/// Our own, as wkt 0.14's `wkt::to_wkt::write_geometry` panics on an empty polygon inside a multi-polygon. Switch to
/// it once a wkt release has <https://github.com/georust/wkt/pull/166>.
pub fn wkt(g: &impl GeometryTrait<T = f64>) -> String {
    use geo_traits::Dimensions as D;
    let tag = match g.dim() {
        D::Xyz => " Z",
        D::Xym => " M",
        D::Xyzm => " ZM",
        D::Xy | D::Unknown(_) => "",
    };
    let coords = |c: Vec<Vec<f64>>| -> String {
        let points: Vec<String> = c
            .iter()
            .map(|o| o.iter().map(f64::to_string).collect::<Vec<_>>().join(" "))
            .collect();
        format!("({})", points.join(","))
    };
    let rings = |r: Vec<Vec<Vec<f64>>>| -> String {
        if r.is_empty() {
            "EMPTY".into()
        } else {
            format!(
                "({})",
                r.into_iter().map(coords).collect::<Vec<_>>().join(",")
            )
        }
    };
    let list = |items: Vec<String>| -> String {
        if items.is_empty() {
            " EMPTY".into()
        } else {
            format!("({})", items.join(","))
        }
    };
    match shape(g) {
        Shape::Point(c) if c.is_empty() => format!("POINT{tag} EMPTY"),
        Shape::Point(c) => format!("POINT{tag}{}", coords(vec![c])),
        Shape::LineString(c) if c.is_empty() => format!("LINESTRING{tag} EMPTY"),
        Shape::LineString(c) => format!("LINESTRING{tag}{}", coords(c)),
        Shape::Polygon(r) if r.is_empty() => format!("POLYGON{tag} EMPTY"),
        Shape::Polygon(r) => format!("POLYGON{tag}{}", rings(r)),
        Shape::MultiPoint(p) => format!(
            "MULTIPOINT{tag}{}",
            list(p.into_iter().map(|c| coords(vec![c])).collect())
        ),
        Shape::MultiLineString(l) => format!(
            "MULTILINESTRING{tag}{}",
            list(
                l.into_iter()
                    .map(|c| if c.is_empty() {
                        "EMPTY".into()
                    } else {
                        coords(c)
                    })
                    .collect()
            )
        ),
        Shape::MultiPolygon(p) => format!(
            "MULTIPOLYGON{tag}{}",
            list(p.into_iter().map(rings).collect())
        ),
        Shape::GeometryCollection(_) => {
            use geo_traits::GeometryCollectionTrait as _;
            let Kind::GeometryCollection(c) = g.as_type() else {
                unreachable!()
            };
            format!(
                "GEOMETRYCOLLECTION{tag}{}",
                list(c.geometries().map(|g| wkt(&g)).collect())
            )
        }
    }
}
