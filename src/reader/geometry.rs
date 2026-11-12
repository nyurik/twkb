//! The geometries a TWKB buffer decodes to, and their [geo-traits](geo_traits).
//!
//! Each owns the stored ordinates of all its coordinates in one vector, with where each ring or part ends. The parts
//! of a multi-geometry and the rings of a polygon are views into it ([`PointRef`], [`LineStringRef`],
//! [`PolygonRef`]), and a coordinate is a view of its ordinates ([`Coord`]).

use geo_traits::{
    CoordTrait, Dimensions, GeometryCollectionTrait, GeometryTrait, GeometryType as Kind,
    LineStringTrait, MultiLineStringTrait, MultiPointTrait, MultiPolygonTrait, PointTrait,
    PolygonTrait, UnimplementedGeometryCollection, UnimplementedLine, UnimplementedLineString,
    UnimplementedMultiLineString, UnimplementedMultiPoint, UnimplementedMultiPolygon,
    UnimplementedPoint, UnimplementedPolygon, UnimplementedRect, UnimplementedTriangle,
};

use super::{Body, Header};
use crate::common::{GeometryType, to_f64};

/// A geometry read from TWKB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Twkb {
    /// A point.
    Point(Point),
    /// A line string.
    LineString(LineString),
    /// A polygon.
    Polygon(Polygon),
    /// Points.
    MultiPoint(MultiPoint),
    /// Line strings.
    MultiLineString(MultiLineString),
    /// Polygons.
    MultiPolygon(MultiPolygon),
    /// Geometries of any type.
    GeometryCollection(GeometryCollection),
}

impl Twkb {
    pub(crate) fn from_parts(header: Header, body: Body) -> Self {
        let Body {
            coords,
            rings,
            parts,
            geometries,
        } = body;
        match header.geometry_type() {
            GeometryType::Point => Self::Point(Point { header, coords }),
            GeometryType::LineString => Self::LineString(LineString { header, coords }),
            GeometryType::Polygon => Self::Polygon(Polygon {
                header,
                coords,
                rings,
            }),
            GeometryType::MultiPoint => Self::MultiPoint(MultiPoint { header, coords }),
            GeometryType::MultiLineString => Self::MultiLineString(MultiLineString {
                header,
                coords,
                lines: parts,
            }),
            GeometryType::MultiPolygon => Self::MultiPolygon(MultiPolygon {
                header,
                coords,
                rings,
                polygons: parts,
            }),
            GeometryType::GeometryCollection => {
                Self::GeometryCollection(GeometryCollection { header, geometries })
            }
        }
    }

    /// The geometry's header.
    #[must_use]
    #[inline]
    pub fn header(&self) -> &Header {
        match self {
            Self::Point(g) => &g.header,
            Self::LineString(g) => &g.header,
            Self::Polygon(g) => &g.header,
            Self::MultiPoint(g) => &g.header,
            Self::MultiLineString(g) => &g.header,
            Self::MultiPolygon(g) => &g.header,
            Self::GeometryCollection(g) => &g.header,
        }
    }

    /// The stored ordinates of every coordinate, in order, as integers at the header's precision; empty for a
    /// collection, whose geometries each have their own.
    #[must_use]
    #[inline]
    pub fn stored_coords(&self) -> &[i64] {
        match self {
            Self::Point(g) => &g.coords,
            Self::LineString(g) => &g.coords,
            Self::Polygon(g) => &g.coords,
            Self::MultiPoint(g) => &g.coords,
            Self::MultiLineString(g) => &g.coords,
            Self::MultiPolygon(g) => &g.coords,
            Self::GeometryCollection(_) => &[],
        }
    }
}

// The accessors here are `#[inline]` so that callers in other crates can inline them: reading a line of 1,000
// points' `f64` ordinates took 11.4 µs without it, 5.8 µs with it (`just bench`, i9-10885H, 2026-11-11)

/// The coordinate `i` of `values`, which holds coordinates of `header`'s dimensions.
#[inline]
fn coord<'a>(values: &'a [i64], header: &'a Header, i: usize) -> Coord<'a> {
    let n = header.dimension().size();
    Coord {
        values: &values[i * n..(i + 1) * n],
        header,
    }
}

/// Coordinates `from..to` of `values`.
#[inline]
fn coords<'a>(values: &'a [i64], header: &'a Header, from: usize, to: usize) -> LineStringRef<'a> {
    let n = header.dimension().size();
    LineStringRef {
        values: &values[from * n..to * n],
        header,
    }
}

/// The start of part `i`, whose parts end at `ends`.
#[inline]
fn start(ends: &[usize], i: usize) -> usize {
    if i == 0 { 0 } else { ends[i - 1] }
}

/// A coordinate: a view of its stored ordinates.
#[derive(Debug, Clone, Copy)]
pub struct Coord<'a> {
    values: &'a [i64],
    header: &'a Header,
}

impl Coord<'_> {
    /// The stored ordinates, as integers at the header's precision.
    #[must_use]
    #[inline]
    pub fn stored(&self) -> &[i64] {
        self.values
    }
}

impl CoordTrait for Coord<'_> {
    type T = f64;

    #[inline]
    fn dim(&self) -> Dimensions {
        self.header.dimension().into()
    }

    #[inline]
    fn x(&self) -> f64 {
        to_f64(self.values[0], self.header.digits[0])
    }

    #[inline]
    fn y(&self) -> f64 {
        to_f64(self.values[1], self.header.digits[1])
    }

    #[inline]
    fn nth_or_panic(&self, n: usize) -> f64 {
        to_f64(self.values[n], self.header.digits[n])
    }
}

/// A point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Point {
    header: Header,
    coords: Vec<i64>,
}

/// A point of a multi-point.
#[derive(Debug, Clone, Copy)]
pub struct PointRef<'a> {
    values: &'a [i64],
    header: &'a Header,
}

/// A line string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineString {
    header: Header,
    coords: Vec<i64>,
}

/// A ring of a polygon, or a line string of a multi-line string.
#[derive(Debug, Clone, Copy)]
pub struct LineStringRef<'a> {
    values: &'a [i64],
    header: &'a Header,
}

/// A polygon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Polygon {
    header: Header,
    coords: Vec<i64>,
    /// The end of each ring, in coordinates.
    rings: Vec<usize>,
}

/// A polygon of a multi-polygon.
#[derive(Debug, Clone, Copy)]
pub struct PolygonRef<'a> {
    /// The coordinates of the multi-polygon.
    values: &'a [i64],
    /// The ends of this polygon's rings, in the multi-polygon's coordinates.
    rings: &'a [usize],
    /// Where its first ring starts.
    start: usize,
    header: &'a Header,
}

/// Points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiPoint {
    header: Header,
    coords: Vec<i64>,
}

/// Line strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiLineString {
    header: Header,
    coords: Vec<i64>,
    /// The end of each line string, in coordinates.
    lines: Vec<usize>,
}

/// Polygons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiPolygon {
    header: Header,
    coords: Vec<i64>,
    /// The end of each ring, in coordinates.
    rings: Vec<usize>,
    /// The end of each polygon, in rings.
    polygons: Vec<usize>,
}

/// Geometries, each with its own header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometryCollection {
    header: Header,
    geometries: Vec<Twkb>,
}

/// The header and stored ordinates of the owned geometries.
macro_rules! owned {
    ($($ty:ident),*) => {$(
        impl $ty {
            /// The geometry's header.
            #[must_use]
            #[inline]
            pub fn header(&self) -> &Header {
                &self.header
            }

            /// The stored ordinates of every coordinate, in order, as integers at the header's precision.
            #[must_use]
            #[inline]
            pub fn stored_coords(&self) -> &[i64] {
                &self.coords
            }
        }
    )*};
}

owned!(
    Point,
    LineString,
    Polygon,
    MultiPoint,
    MultiLineString,
    MultiPolygon
);

impl GeometryCollection {
    /// The collection's header.
    #[must_use]
    #[inline]
    pub fn header(&self) -> &Header {
        &self.header
    }
}

impl PointTrait for Point {
    type CoordType<'a> = Coord<'a>;

    #[inline]
    fn coord(&self) -> Option<Coord<'_>> {
        (!self.coords.is_empty()).then(|| coord(&self.coords, &self.header, 0))
    }
}

impl<'a> PointTrait for PointRef<'a> {
    type CoordType<'b>
        = Coord<'a>
    where
        Self: 'b;

    #[inline]
    fn coord(&self) -> Option<Coord<'a>> {
        Some(Coord {
            values: self.values,
            header: self.header,
        })
    }
}

impl LineStringTrait for LineString {
    type CoordType<'a> = Coord<'a>;

    #[inline]
    fn num_coords(&self) -> usize {
        self.coords.len() / self.header.dimension().size()
    }

    #[expect(
        unsafe_code,
        reason = "geo-traits declares it unsafe; the index is checked"
    )]
    #[inline]
    unsafe fn coord_unchecked(&self, i: usize) -> Coord<'_> {
        coord(&self.coords, &self.header, i)
    }
}

impl<'a> LineStringTrait for LineStringRef<'a> {
    type CoordType<'b>
        = Coord<'a>
    where
        Self: 'b;

    #[inline]
    fn num_coords(&self) -> usize {
        self.values.len() / self.header.dimension().size()
    }

    #[expect(
        unsafe_code,
        reason = "geo-traits declares it unsafe; the index is checked"
    )]
    #[inline]
    unsafe fn coord_unchecked(&self, i: usize) -> Coord<'a> {
        coord(self.values, self.header, i)
    }
}

impl PolygonTrait for Polygon {
    type RingType<'a> = LineStringRef<'a>;

    #[inline]
    fn exterior(&self) -> Option<LineStringRef<'_>> {
        (!self.rings.is_empty()).then(|| coords(&self.coords, &self.header, 0, self.rings[0]))
    }

    #[inline]
    fn num_interiors(&self) -> usize {
        self.rings.len().saturating_sub(1)
    }

    #[expect(
        unsafe_code,
        reason = "geo-traits declares it unsafe; the index is checked"
    )]
    #[inline]
    unsafe fn interior_unchecked(&self, i: usize) -> LineStringRef<'_> {
        coords(&self.coords, &self.header, self.rings[i], self.rings[i + 1])
    }
}

impl<'a> PolygonTrait for PolygonRef<'a> {
    type RingType<'b>
        = LineStringRef<'a>
    where
        Self: 'b;

    #[inline]
    fn exterior(&self) -> Option<LineStringRef<'a>> {
        (!self.rings.is_empty())
            .then(|| coords(self.values, self.header, self.start, self.rings[0]))
    }

    #[inline]
    fn num_interiors(&self) -> usize {
        self.rings.len().saturating_sub(1)
    }

    #[expect(
        unsafe_code,
        reason = "geo-traits declares it unsafe; the index is checked"
    )]
    #[inline]
    unsafe fn interior_unchecked(&self, i: usize) -> LineStringRef<'a> {
        coords(self.values, self.header, self.rings[i], self.rings[i + 1])
    }
}

impl MultiPointTrait for MultiPoint {
    type InnerPointType<'a> = PointRef<'a>;

    #[inline]
    fn num_points(&self) -> usize {
        self.coords.len() / self.header.dimension().size()
    }

    #[expect(
        unsafe_code,
        reason = "geo-traits declares it unsafe; the index is checked"
    )]
    #[inline]
    unsafe fn point_unchecked(&self, i: usize) -> PointRef<'_> {
        let c = coord(&self.coords, &self.header, i);
        PointRef {
            values: c.values,
            header: c.header,
        }
    }
}

impl MultiLineStringTrait for MultiLineString {
    type InnerLineStringType<'a> = LineStringRef<'a>;

    #[inline]
    fn num_line_strings(&self) -> usize {
        self.lines.len()
    }

    #[expect(
        unsafe_code,
        reason = "geo-traits declares it unsafe; the index is checked"
    )]
    #[inline]
    unsafe fn line_string_unchecked(&self, i: usize) -> LineStringRef<'_> {
        coords(
            &self.coords,
            &self.header,
            start(&self.lines, i),
            self.lines[i],
        )
    }
}

impl MultiPolygonTrait for MultiPolygon {
    type InnerPolygonType<'a> = PolygonRef<'a>;

    #[inline]
    fn num_polygons(&self) -> usize {
        self.polygons.len()
    }

    #[expect(
        unsafe_code,
        reason = "geo-traits declares it unsafe; the index is checked"
    )]
    #[inline]
    unsafe fn polygon_unchecked(&self, i: usize) -> PolygonRef<'_> {
        let first = start(&self.polygons, i);
        PolygonRef {
            values: &self.coords,
            rings: &self.rings[first..self.polygons[i]],
            start: start(&self.rings, first),
            header: &self.header,
        }
    }
}

impl GeometryCollectionTrait for GeometryCollection {
    type GeometryType<'a> = &'a Twkb;

    #[inline]
    fn num_geometries(&self) -> usize {
        self.geometries.len()
    }

    #[expect(
        unsafe_code,
        reason = "geo-traits declares it unsafe; the index is checked"
    )]
    #[inline]
    unsafe fn geometry_unchecked(&self, i: usize) -> &Twkb {
        &self.geometries[i]
    }
}

/// The associated type of a geometry trait: the type itself in its own slot, unimplemented in the others.
macro_rules! slot {
    (Point, Point, $t:ty, $u:ty) => {
        $t
    };
    (LineString, LineString, $t:ty, $u:ty) => {
        $t
    };
    (Polygon, Polygon, $t:ty, $u:ty) => {
        $t
    };
    (MultiPoint, MultiPoint, $t:ty, $u:ty) => {
        $t
    };
    (MultiLineString, MultiLineString, $t:ty, $u:ty) => {
        $t
    };
    (MultiPolygon, MultiPolygon, $t:ty, $u:ty) => {
        $t
    };
    (GeometryCollection, GeometryCollection, $t:ty, $u:ty) => {
        $t
    };
    ($a:ident, $b:ident, $t:ty, $u:ty) => {
        $u
    };
}

/// [`GeometryTrait`] for a geometry of one type.
macro_rules! geometry_trait {
    ($ty:ty, $kind:ident $(, $lt:lifetime)?) => {
        impl$(<$lt>)? GeometryTrait for $ty {
            type T = f64;
            type PointType<'b> = slot!($kind, Point, Self, UnimplementedPoint<f64>) where Self: 'b;
            type LineStringType<'b> = slot!($kind, LineString, Self, UnimplementedLineString<f64>) where Self: 'b;
            type PolygonType<'b> = slot!($kind, Polygon, Self, UnimplementedPolygon<f64>) where Self: 'b;
            type MultiPointType<'b> = slot!($kind, MultiPoint, Self, UnimplementedMultiPoint<f64>) where Self: 'b;
            type MultiLineStringType<'b>
                = slot!($kind, MultiLineString, Self, UnimplementedMultiLineString<f64>) where Self: 'b;
            type MultiPolygonType<'b> = slot!($kind, MultiPolygon, Self, UnimplementedMultiPolygon<f64>) where Self: 'b;
            type GeometryCollectionType<'b>
                = slot!($kind, GeometryCollection, Self, UnimplementedGeometryCollection<f64>) where Self: 'b;
            type RectType<'b> = UnimplementedRect<f64> where Self: 'b;
            type TriangleType<'b> = UnimplementedTriangle<f64> where Self: 'b;
            type LineType<'b> = UnimplementedLine<f64> where Self: 'b;

            #[inline]
            fn dim(&self) -> Dimensions {
                self.header.dimension().into()
            }

            #[inline]
            fn as_type(
                &self,
            ) -> Kind<
                '_,
                Self::PointType<'_>,
                Self::LineStringType<'_>,
                Self::PolygonType<'_>,
                Self::MultiPointType<'_>,
                Self::MultiLineStringType<'_>,
                Self::MultiPolygonType<'_>,
                Self::GeometryCollectionType<'_>,
                Self::RectType<'_>,
                Self::TriangleType<'_>,
                Self::LineType<'_>,
            > {
                Kind::$kind(self)
            }
        }
    };
}

geometry_trait!(Point, Point);
geometry_trait!(PointRef<'a>, Point, 'a);
geometry_trait!(LineString, LineString);
geometry_trait!(LineStringRef<'a>, LineString, 'a);
geometry_trait!(Polygon, Polygon);
geometry_trait!(PolygonRef<'a>, Polygon, 'a);
geometry_trait!(MultiPoint, MultiPoint);
geometry_trait!(MultiLineString, MultiLineString);
geometry_trait!(MultiPolygon, MultiPolygon);
geometry_trait!(GeometryCollection, GeometryCollection);

/// [`GeometryTrait`] for a geometry of any type, owned or borrowed.
macro_rules! any_geometry_trait {
    ($ty:ty $(, $lt:lifetime)?) => {
        impl$(<$lt>)? GeometryTrait for $ty {
            type T = f64;
            type PointType<'b> = Point where Self: 'b;
            type LineStringType<'b> = LineString where Self: 'b;
            type PolygonType<'b> = Polygon where Self: 'b;
            type MultiPointType<'b> = MultiPoint where Self: 'b;
            type MultiLineStringType<'b> = MultiLineString where Self: 'b;
            type MultiPolygonType<'b> = MultiPolygon where Self: 'b;
            type GeometryCollectionType<'b> = GeometryCollection where Self: 'b;
            type RectType<'b> = UnimplementedRect<f64> where Self: 'b;
            type TriangleType<'b> = UnimplementedTriangle<f64> where Self: 'b;
            type LineType<'b> = UnimplementedLine<f64> where Self: 'b;

            #[inline]
            fn dim(&self) -> Dimensions {
                self.header().dimension().into()
            }

            #[inline]
            fn as_type(
                &self,
            ) -> Kind<
                '_,
                Point,
                LineString,
                Polygon,
                MultiPoint,
                MultiLineString,
                MultiPolygon,
                GeometryCollection,
                UnimplementedRect<f64>,
                UnimplementedTriangle<f64>,
                UnimplementedLine<f64>,
            > {
                match self {
                    Twkb::Point(g) => Kind::Point(g),
                    Twkb::LineString(g) => Kind::LineString(g),
                    Twkb::Polygon(g) => Kind::Polygon(g),
                    Twkb::MultiPoint(g) => Kind::MultiPoint(g),
                    Twkb::MultiLineString(g) => Kind::MultiLineString(g),
                    Twkb::MultiPolygon(g) => Kind::MultiPolygon(g),
                    Twkb::GeometryCollection(g) => Kind::GeometryCollection(g),
                }
            }
        }
    };
}

any_geometry_trait!(Twkb);
any_geometry_trait!(&'a Twkb, 'a);
