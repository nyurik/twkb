//! The checks of the fuzz targets, kept in a library so a failing input can be run as a test.

use geo_traits::{
    CoordTrait as _, GeometryCollectionTrait as _, GeometryTrait, GeometryType as Kind,
    LineStringTrait as _, LineTrait as _, MultiLineStringTrait as _, MultiPointTrait as _,
    MultiPolygonTrait as _, PointTrait as _, PolygonTrait as _,
};
use twkb::{Precision, ReadOptions, Twkb, TwkbError, WriteOptions, write_geometry};

/// Small enough that a fuzzer's input cannot exhaust the memory of its process.
const LIMITS: ReadOptions = ReadOptions {
    max_bytes: 1 << 24,
    max_nesting: 32,
};

/// The largest magnitude written from `f64` and read back exactly: two roundings stay below half a unit.
#[expect(clippy::cast_precision_loss, reason = "2^50 is exact")]
const EXACT: f64 = (1_u64 << 50) as f64;

/// Reads `data`; if it is a geometry whose ordinates are exact as `f64`, writes it again and reads it back.
///
/// # Panics
///
/// When what is read cannot be written, or is written as another geometry.
pub fn read(data: &[u8]) {
    let Ok(g) = Twkb::new_with(data, &LIMITS) else {
        return;
    };
    if !exact(&g) {
        return;
    }
    let header = g.header();
    let opts = WriteOptions {
        precision: header.precision(),
        size: header.size().is_some(),
        bbox: header.bbox_stored().is_some(),
        remove_repeated_points: false,
    };
    let mut buf = Vec::new();
    match write_geometry(&mut buf, &g, &opts) {
        Ok(()) => {}
        // A collection's geometries are written with its precision, which may scale them beyond 64 bits
        Err(TwkbError::OutOfRange { .. }) if !uniform(&g, header.precision()) => return,
        Err(e) => panic!("a geometry read can be written: {e:?}"),
    }
    let again = Twkb::new_with(&buf, &LIMITS).expect("a geometry written can be read");
    // The writer gives a collection's geometries the collection's precision, so only then do the stored values match
    if uniform(&g, header.precision()) {
        assert_eq!(tree(&g), tree(&again), "{data:02x?} written as {buf:02x?}");
    }
}

/// Whether every geometry inside `g` has the precision `p`.
fn uniform(g: &Twkb, p: Precision) -> bool {
    g.header().precision() == p
        && match g {
            Twkb::GeometryCollection(c) => c.geometries().all(|g| uniform(g, p)),
            _ => true,
        }
}

/// Whether every stored ordinate of `g` comes back the same through `f64`.
#[expect(
    clippy::cast_precision_loss,
    reason = "a value beyond 2^53 is not exact, and is compared as not"
)]
fn exact(g: &Twkb) -> bool {
    let digits = g.header().precision().xy.min(0).unsigned_abs();
    let limit = EXACT / 10_f64.powi(digits.into());
    let fits = g.stored_coords().iter().all(|&v| (v as f64).abs() < limit);
    match g {
        Twkb::GeometryCollection(c) => fits && c.geometries().all(exact),
        _ => fits,
    }
}

/// A geometry's type, structure and stored ordinates; an empty geometry is only its type.
#[derive(Debug, PartialEq)]
enum Tree {
    Empty(&'static str),
    Coords(&'static str, Vec<usize>, Vec<i64>),
    Collection(Vec<Tree>),
}

fn tree(g: &Twkb) -> Tree {
    // A polygon whose rings have no points is written with no rings, as geo-types has an empty polygon
    fn rings(p: &impl geo_traits::PolygonTrait<T = f64>) -> Vec<usize> {
        let rings: Vec<usize> = p
            .exterior()
            .into_iter()
            .chain(p.interiors())
            .map(|r| r.num_coords())
            .collect();
        if rings.iter().all(|&n| n == 0) {
            Vec::new()
        } else {
            rings
        }
    }
    let parts = |g: &Twkb| -> Vec<usize> {
        match g.as_type() {
            Kind::Polygon(p) => rings(p),
            Kind::MultiLineString(m) => m.line_strings().map(|l| l.num_coords()).collect(),
            Kind::MultiPolygon(m) => m
                .polygons()
                .flat_map(|p| {
                    let rings = rings(&p);
                    std::iter::once(rings.len()).chain(rings)
                })
                .collect(),
            _ => Vec::new(),
        }
    };
    let name = match g {
        Twkb::Point(_) => "point",
        Twkb::LineString(_) => "line string",
        Twkb::Polygon(_) => "polygon",
        Twkb::MultiPoint(_) => "multi-point",
        Twkb::MultiLineString(_) => "multi-line string",
        Twkb::MultiPolygon(_) => "multi-polygon",
        // A collection of only empty geometries is written empty, as PostGIS writes it
        Twkb::GeometryCollection(c) => {
            let children: Vec<Tree> = c.geometries().map(tree).collect();
            let empty = |t: &Tree| {
                matches!(t, Tree::Empty(_)) || matches!(t, Tree::Collection(c) if c.is_empty())
            };
            return Tree::Collection(if children.iter().all(empty) {
                Vec::new()
            } else {
                children
            });
        }
    };
    if g.stored_coords().is_empty() && parts(g).iter().all(|&n| n == 0) {
        Tree::Empty(name)
    } else {
        Tree::Coords(name, parts(g), g.stored_coords().to_vec())
    }
}

type Xy = (f64, f64);

/// A geometry of one type, as the write target builds it: geo-types' own `Arbitrary` recurses into collections
/// without bound, and hangs on short inputs.
#[derive(Debug, arbitrary::Arbitrary)]
pub enum Part {
    Point(Xy),
    LineString(Vec<Xy>),
    Polygon(Vec<Vec<Xy>>),
    MultiPoint(Vec<Xy>),
    MultiLineString(Vec<Vec<Xy>>),
    MultiPolygon(Vec<Vec<Vec<Xy>>>),
    Rect(Xy, Xy),
}

/// A geometry, or a collection of them.
#[derive(Debug, arbitrary::Arbitrary)]
pub enum Shape {
    One(Part),
    Collection(Vec<Part>),
}

fn line(c: &[Xy]) -> geo_types::LineString {
    c.iter().copied().collect()
}

fn polygon(rings: &[Vec<Xy>]) -> geo_types::Polygon {
    let mut rings = rings.iter().map(|r| line(r));
    geo_types::Polygon::new(
        rings
            .next()
            .unwrap_or_else(|| geo_types::LineString::new(Vec::new())),
        rings.collect(),
    )
}

impl Part {
    fn geometry(&self) -> geo_types::Geometry {
        use geo_types::{Geometry as G, MultiLineString, MultiPoint, MultiPolygon, Point, Rect};
        match self {
            Self::Point(c) => G::Point(Point::from(*c)),
            Self::LineString(c) => G::LineString(line(c)),
            Self::Polygon(r) => G::Polygon(polygon(r)),
            Self::MultiPoint(c) => {
                G::MultiPoint(MultiPoint::new(c.iter().map(|&c| Point::from(c)).collect()))
            }
            Self::MultiLineString(l) => {
                G::MultiLineString(MultiLineString::new(l.iter().map(|l| line(l)).collect()))
            }
            Self::MultiPolygon(p) => {
                G::MultiPolygon(MultiPolygon::new(p.iter().map(|p| polygon(p)).collect()))
            }
            Self::Rect(a, b) => G::Rect(Rect::new(*a, *b)),
        }
    }
}

impl Shape {
    fn geometry(&self) -> geo_types::Geometry {
        match self {
            Self::One(p) => p.geometry(),
            Self::Collection(parts) => {
                geo_types::Geometry::GeometryCollection(parts.iter().map(Part::geometry).collect())
            }
        }
    }
}

/// A geometry and the options to write it with.
#[derive(Debug, arbitrary::Arbitrary)]
pub struct WriteInput {
    shape: Shape,
    xy: i8,
    size: bool,
    bbox: bool,
    remove_repeated_points: bool,
}

impl WriteInput {
    /// Writes the geometry; if its ordinates are finite and small enough to be exact, reads it back.
    ///
    /// # Panics
    ///
    /// When writing fails other than for a rectangle or an ordinate out of range, when what is written cannot be
    /// read, or when it reads back beyond its precision.
    pub fn check(&self) {
        let xy = self.xy.rem_euclid(16) - 8;
        let opts = WriteOptions {
            precision: Precision::xy(xy),
            size: self.size,
            bbox: self.bbox,
            remove_repeated_points: self.remove_repeated_points,
        };
        let geometry = self.shape.geometry();
        let scale = 10_f64.powi(xy.into());
        let mut ordinates = Vec::new();
        ordinates_of(&geometry, &mut ordinates);
        let mut buf = Vec::new();
        match write_geometry(&mut buf, &geometry, &opts) {
            Ok(()) => {}
            Err(TwkbError::OutOfRange { value, .. }) => {
                // Not finite, or beyond 2^63 once scaled; the bound is loose for the rounding of the scaling
                let scaled = (value * scale).abs();
                assert!(
                    scaled.is_nan() || scaled >= 9.2e18,
                    "{value} at {xy} digits"
                );
                return;
            }
            Err(TwkbError::Unsupported(_)) => {
                let rect = |p: &Part| matches!(p, Part::Rect(..));
                let has_rect = match &self.shape {
                    Shape::One(p) => rect(p),
                    Shape::Collection(parts) => parts.iter().any(rect),
                };
                assert!(has_rect, "{geometry:?}");
                return;
            }
            Err(e) => panic!("{e}"),
        }
        let read = Twkb::new_with(&buf, &LIMITS).expect("a geometry written can be read");
        if self.remove_repeated_points
            || !ordinates
                .iter()
                .all(|v| v.is_finite() && (v * scale).abs() < EXACT)
        {
            return;
        }
        let mut back = Vec::new();
        ordinates_of(&read, &mut back);
        assert_eq!(ordinates.len(), back.len(), "{geometry:?}");
        let half = 0.5 / scale * (1.0 + 1e-9);
        for (a, b) in ordinates.iter().zip(&back) {
            assert!(
                (a - b).abs() <= half + a.abs() * 1e-15,
                "{a} read back as {b} at {xy} digits"
            );
        }
    }
}

/// Every ordinate of `g`, in order.
fn ordinates_of(g: &impl GeometryTrait<T = f64>, out: &mut Vec<f64>) {
    let mut coord = |c: &dyn Fn(usize) -> f64| out.extend([c(0), c(1)]);
    match g.as_type() {
        Kind::Point(p) => {
            if let Some(c) = p.coord() {
                coord(&|n| c.nth_or_panic(n));
            }
        }
        Kind::LineString(l) => l.coords().for_each(|c| coord(&|n| c.nth_or_panic(n))),
        Kind::Line(l) => [l.start(), l.end()]
            .iter()
            .for_each(|c| coord(&|n| c.nth_or_panic(n))),
        // A polygon with no exterior is empty, whatever geo-traits gives as its interiors
        Kind::Polygon(p) => {
            if let Some(e) = p.exterior() {
                for r in std::iter::once(e).chain(p.interiors()) {
                    r.coords().for_each(|c| coord(&|n| c.nth_or_panic(n)));
                }
            }
        }
        Kind::MultiPoint(m) => m.points().for_each(|p| {
            if let Some(c) = p.coord() {
                coord(&|n| c.nth_or_panic(n));
            }
        }),
        Kind::MultiLineString(m) => m
            .line_strings()
            .for_each(|l| l.coords().for_each(|c| coord(&|n| c.nth_or_panic(n)))),
        Kind::MultiPolygon(m) => m.polygons().for_each(|p| {
            if let Some(e) = p.exterior() {
                for r in std::iter::once(e).chain(p.interiors()) {
                    r.coords().for_each(|c| coord(&|n| c.nth_or_panic(n)));
                }
            }
        }),
        Kind::GeometryCollection(c) => c.geometries().for_each(|g| ordinates_of(&g, out)),
        Kind::Rect(_) | Kind::Triangle(_) => {}
    }
}
