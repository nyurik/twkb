//! Writing geometries that implement [geo-traits](geo_traits) as TWKB.
//!
//! TWKB writes counts and sizes before what they count, so a writer appends to a `Vec<u8>` and fills them in after
//! the fact where it must: the size when [`WriteOptions::size`] is set, and a count of points when repeated points
//! are removed. [`write_scaled_point`] and [`write_scaled_line_string`] take coordinates that are integers already,
//! as an integer source such as OpenStreetMap has them, with nothing to round or check.

use geo_traits::{
    CoordTrait, GeometryCollectionTrait as _, GeometryTrait, GeometryType as Kind,
    LineStringTrait as _, LineTrait as _, MultiLineStringTrait as _, MultiPointTrait,
    MultiPolygonTrait as _, PointTrait as _, PolygonTrait,
};

use crate::common::{
    BBOX, Dimension, EMPTY, EXTENDED, GeometryType, Precision, SIZE, to_stored, write_svarint,
    write_uvarint, zigzag,
};
use crate::error::TwkbError;

/// How to write a geometry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WriteOptions {
    /// The decimal digits each ordinate keeps: the same for every geometry of a collection, as the format writes
    /// them in each geometry's header but PostGIS uses one for all.
    pub precision: Precision,
    /// Whether the header gives the size of the rest of the geometry, so a reader can skip it.
    pub size: bool,
    /// Whether the header gives the bounding box. An empty geometry has none.
    pub bbox: bool,
    /// Whether a point equal to the one before it once rounded is left out of a line string or a ring, as PostGIS's
    /// [`ST_AsTWKB`](https://postgis.net/docs/ST_AsTWKB.html) does, keeping at least 2 points of a line string and 4
    /// of a ring. Off by default, so every point is written; the points of a multi-point are always all written.
    pub remove_repeated_points: bool,
}

/// Appends `geom` as TWKB.
///
/// An ordinate that is not finite, or too large for an `i64` once scaled, is an error ([`TwkbError::OutOfRange`]),
/// as is a rectangle or a triangle: TWKB has neither, and [`geo_traits`] leaves the choice of their rings to the
/// caller. A line is written as a line string. As PostGIS writes them, an empty point of a multi-point is left out, and
/// a multi-geometry or a collection of only empty parts is empty. An empty geometry keeps its dimensions, except that
/// one of x and y only takes the dimensions of the collection it is in. On an error, `out` is left as it was.
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "options by reference, as wkb's writer takes them"
)]
pub fn write_geometry(
    out: &mut Vec<u8>,
    geom: &impl GeometryTrait<T = f64>,
    opts: &WriteOptions,
) -> Result<(), TwkbError> {
    opts.precision.check()?;
    let start = out.len();
    let written = write_any(out, geom, *opts, None);
    if written.is_err() {
        out.truncate(start);
    }
    written
}

/// Appends a point whose x and y are integers at `precision` decimal digits (`round(x * 10^precision)`), with no
/// size or bounding box.
pub fn write_scaled_point(
    out: &mut Vec<u8>,
    precision: i8,
    [x, y]: [i64; 2],
) -> Result<(), TwkbError> {
    Precision::xy(precision).check()?;
    out.extend([first_byte(GeometryType::Point, precision), 0]);
    write_svarint(out, x);
    write_svarint(out, y);
    Ok(())
}

/// Appends a line string whose x and y are integers at `precision` decimal digits (`round(x * 10^precision)`), with
/// no size or bounding box, and every point kept; no points is an empty line string. The count is the iterator's
/// `len()`, written first: an iterator that yields another number of points is an error
/// ([`TwkbError::LengthMismatch`]), and nothing is appended.
pub fn write_scaled_line_string(
    out: &mut Vec<u8>,
    precision: i8,
    coords: impl ExactSizeIterator<Item = [i64; 2]>,
) -> Result<(), TwkbError> {
    Precision::xy(precision).check()?;
    let count = coords.len();
    let start = out.len();
    if count == 0 {
        out.extend([first_byte(GeometryType::LineString, precision), EMPTY]);
    } else {
        out.extend([first_byte(GeometryType::LineString, precision), 0]);
        out.reserve(1 + count * 4);
        write_uvarint(out, count as u64);
    }
    let (mut lx, mut ly) = (0_i64, 0_i64);
    let mut written = 0;
    for [x, y] in coords {
        write_svarint(out, x.wrapping_sub(lx));
        write_svarint(out, y.wrapping_sub(ly));
        (lx, ly) = (x, y);
        written += 1;
    }
    if written != count {
        out.truncate(start);
        return Err(TwkbError::LengthMismatch {
            len: count,
            items: written,
        });
    }
    Ok(())
}

/// Inserts `v` as an unsigned varint at `at`, for a count or a size known only after what follows it.
#[expect(clippy::cast_possible_truncation, reason = "7 bits at a time")]
fn insert_uvarint(out: &mut Vec<u8>, at: usize, v: u64) {
    let mut buf = [0; 10];
    let mut n = 0;
    let mut v = v;
    while v >= 0x80 {
        buf[n] = (v & 0x7f) as u8 | 0x80;
        v >>= 7;
        n += 1;
    }
    buf[n] = v as u8;
    out.splice(at..at, buf[..=n].iter().copied());
}

/// The header's first byte: the type, and x and y's precision as a 4-bit zigzag value.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a precision of -8 to 7 is 4 bits as zigzag"
)]
fn first_byte(kind: GeometryType, precision: i8) -> u8 {
    kind as u8 | ((zigzag(precision.into()) as u8) << 4)
}

/// What a geometry is, for its header.
fn classify(geom: &impl GeometryTrait<T = f64>) -> Result<(GeometryType, bool), TwkbError> {
    Ok(match geom.as_type() {
        Kind::Point(g) => (GeometryType::Point, g.coord().is_none()),
        Kind::LineString(g) => (GeometryType::LineString, g.num_coords() == 0),
        Kind::Polygon(g) => (GeometryType::Polygon, polygon_is_empty(g)),
        Kind::MultiPoint(g) => (GeometryType::MultiPoint, points_with_coords(g) == 0),
        Kind::MultiLineString(g) => (
            GeometryType::MultiLineString,
            g.line_strings().all(|l| l.num_coords() == 0),
        ),
        Kind::MultiPolygon(g) => (
            GeometryType::MultiPolygon,
            g.polygons().all(|p| polygon_is_empty(&p)),
        ),
        Kind::GeometryCollection(g) => (
            GeometryType::GeometryCollection,
            g.geometries().all(|g| is_empty(&g)),
        ),
        Kind::Line(_) => (GeometryType::LineString, false),
        Kind::Rect(_) => return Err(TwkbError::Unsupported("rectangle")),
        Kind::Triangle(_) => return Err(TwkbError::Unsupported("triangle")),
    })
}

/// Whether a geometry is empty: as PostGIS has it, a multi-geometry or a collection of only empty parts is.
fn is_empty(geom: &impl GeometryTrait<T = f64>) -> bool {
    classify(geom).is_ok_and(|(_, empty)| empty)
}

/// The points of a multi-point that are not empty.
fn points_with_coords(g: &impl MultiPointTrait<T = f64>) -> usize {
    g.points().filter(|p| p.coord().is_some()).count()
}

/// Whether a polygon is empty: no rings, or as geo-types has it, an exterior of no points and no interiors.
fn polygon_is_empty(g: &impl PolygonTrait<T = f64>) -> bool {
    g.exterior()
        .is_none_or(|r| r.num_coords() == 0 && g.num_interiors() == 0)
}

/// Appends a geometry, header and body; an empty one of x and y only has `empty_dim`, the dimensions of the
/// collection it is in.
fn write_any(
    out: &mut Vec<u8>,
    geom: &impl GeometryTrait<T = f64>,
    opts: WriteOptions,
    empty_dim: Option<Dimension>,
) -> Result<(), TwkbError> {
    let (kind, empty) = classify(geom)?;
    let own = Dimension::try_from(geom.dim())?;
    let dim = if empty && own == Dimension::Xy {
        empty_dim.unwrap_or_default()
    } else {
        own
    };
    let p = opts.precision;
    let mut flags = 0;
    if opts.bbox && !empty {
        flags |= BBOX;
    }
    if opts.size {
        flags |= SIZE;
    }
    if dim != Dimension::Xy {
        flags |= EXTENDED;
    }
    if empty {
        flags |= EMPTY;
    }
    out.extend([first_byte(kind, p.xy), flags]);
    if dim != Dimension::Xy {
        out.push(u8::from(dim.has_z()) | (u8::from(dim.has_m()) << 1) | (p.z << 2) | (p.m << 5));
    }
    let start = out.len();
    if !empty {
        let mut w = Coords {
            opts,
            digits: p.digits(dim),
            n: dim.size(),
            last: [0; 4],
        };
        if opts.bbox {
            w.write_bbox(out, geom)?;
        }
        w.write_body(out, geom, dim)?;
    }
    if opts.size {
        insert_uvarint(out, start, (out.len() - start) as u64);
    }
    Ok(())
}

/// Writes coordinates, each ordinate the step from the one before, across all the parts of a geometry.
struct Coords {
    opts: WriteOptions,
    digits: [i8; 4],
    n: usize,
    last: [i64; 4],
}

impl Coords {
    /// A coordinate's stored ordinates.
    #[inline(always)]
    #[expect(
        clippy::inline_always,
        reason = "measured with `just bench`: see `write`, the other hot call"
    )]
    fn stored(&self, c: &impl CoordTrait<T = f64>) -> Result<[i64; 4], TwkbError> {
        if self.n == 2 {
            return Ok([
                to_stored(c.x(), self.digits[0])?,
                to_stored(c.y(), self.digits[1])?,
                0,
                0,
            ]);
        }
        let mut q = [0; 4];
        for (d, q) in q.iter_mut().enumerate().take(self.n) {
            *q = to_stored(c.nth_or_panic(d), self.digits[d])?;
        }
        Ok(q)
    }

    /// Appends one coordinate.
    #[inline(always)]
    #[expect(
        clippy::inline_always,
        reason = "measured with `just bench`: a line of 1,000 points from geo-types took 19.5 µs with `#[inline]`, 13.4 µs with this and `stored` always inlined"
    )]
    fn write(&mut self, out: &mut Vec<u8>, q: [i64; 4]) {
        if self.n == 2 {
            write_svarint(out, q[0].wrapping_sub(self.last[0]));
            write_svarint(out, q[1].wrapping_sub(self.last[1]));
            self.last[..2].copy_from_slice(&q[..2]);
            return;
        }
        for (d, &q) in q.iter().enumerate().take(self.n) {
            write_svarint(out, q.wrapping_sub(self.last[d]));
            self.last[d] = q;
        }
    }

    /// Appends a count of points, then the points; leaves out a repeated one if the options say so, while that
    /// keeps more than `min_left` points.
    fn write_points<C: CoordTrait<T = f64>>(
        &mut self,
        out: &mut Vec<u8>,
        coords: impl ExactSizeIterator<Item = C>,
        min_left: usize,
    ) -> Result<(), TwkbError> {
        let count = coords.len();
        out.reserve(1 + count * self.n * 2);
        if !self.opts.remove_repeated_points {
            write_uvarint(out, count as u64);
            for c in coords {
                let q = self.stored(&c)?;
                self.write(out, q);
            }
            return Ok(());
        }
        let start = out.len();
        let mut written = 0;
        for (i, c) in coords.enumerate() {
            let q = self.stored(&c)?;
            let left = count - i - 1;
            if i > 0 && q[..self.n] == self.last[..self.n] && written + left > min_left {
                continue;
            }
            self.write(out, q);
            written += 1;
        }
        insert_uvarint(out, start, written as u64);
        Ok(())
    }

    /// Appends the rings, every one as it is, unless the polygon is empty.
    fn write_polygon(
        &mut self,
        out: &mut Vec<u8>,
        g: &impl PolygonTrait<T = f64>,
    ) -> Result<(), TwkbError> {
        let Some(exterior) = g.exterior().filter(|_| !polygon_is_empty(g)) else {
            write_uvarint(out, 0);
            return Ok(());
        };
        write_uvarint(out, 1 + g.num_interiors() as u64);
        self.write_points(out, exterior.coords(), 3)?;
        for ring in g.interiors() {
            self.write_points(out, ring.coords(), 3)?;
        }
        Ok(())
    }

    fn write_body(
        &mut self,
        out: &mut Vec<u8>,
        geom: &impl GeometryTrait<T = f64>,
        dim: Dimension,
    ) -> Result<(), TwkbError> {
        match geom.as_type() {
            Kind::Point(g) => {
                if let Some(c) = g.coord() {
                    let q = self.stored(&c)?;
                    self.write(out, q);
                }
            }
            // A line string keeps at least 2 points when repeated ones are left out: more than 1 left
            Kind::LineString(g) => self.write_points(out, g.coords(), 1)?,
            Kind::Line(g) => self.write_points(out, [g.start(), g.end()].into_iter(), 1)?,
            Kind::Polygon(g) => self.write_polygon(out, g)?,
            Kind::MultiPoint(g) => {
                write_uvarint(out, points_with_coords(g) as u64);
                for p in g.points() {
                    if let Some(c) = p.coord() {
                        let q = self.stored(&c)?;
                        self.write(out, q);
                    }
                }
            }
            Kind::MultiLineString(g) => {
                write_uvarint(out, g.num_line_strings() as u64);
                for line in g.line_strings() {
                    self.write_points(out, line.coords(), 1)?;
                }
            }
            Kind::MultiPolygon(g) => {
                write_uvarint(out, g.num_polygons() as u64);
                for polygon in g.polygons() {
                    self.write_polygon(out, &polygon)?;
                }
            }
            Kind::GeometryCollection(g) => {
                write_uvarint(out, g.num_geometries() as u64);
                for child in g.geometries() {
                    write_any(out, &child, self.opts, Some(dim))?;
                }
            }
            Kind::Rect(_) | Kind::Triangle(_) => unreachable!("classify refuses them"),
        }
        Ok(())
    }

    /// Appends the bounding box: each ordinate's minimum, then the step to its maximum.
    fn write_bbox(
        &self,
        out: &mut Vec<u8>,
        geom: &impl GeometryTrait<T = f64>,
    ) -> Result<(), TwkbError> {
        let mut bbox = Bbox {
            min: [i64::MAX; 4],
            max: [i64::MIN; 4],
        };
        self.extend(&mut bbox, geom)?;
        for d in 0..self.n {
            // An ordinate none of the coordinates has spans 0 to 0
            let (min, max) = if bbox.min[d] <= bbox.max[d] {
                (bbox.min[d], bbox.max[d])
            } else {
                (0, 0)
            };
            write_svarint(out, min);
            write_svarint(out, max.wrapping_sub(min));
        }
        Ok(())
    }

    /// Widens `bbox` to every coordinate of `geom`.
    fn extend(&self, bbox: &mut Bbox, geom: &impl GeometryTrait<T = f64>) -> Result<(), TwkbError> {
        match geom.as_type() {
            Kind::Point(g) => self.extend_points(bbox, g.coord().into_iter()),
            Kind::LineString(g) => self.extend_points(bbox, g.coords()),
            Kind::Line(g) => self.extend_points(bbox, [g.start(), g.end()].into_iter()),
            Kind::Polygon(g) => self.extend_polygon(bbox, g),
            Kind::MultiPoint(g) => g
                .points()
                .try_for_each(|p| self.extend_points(bbox, p.coord().into_iter())),
            Kind::MultiLineString(g) => g
                .line_strings()
                .try_for_each(|l| self.extend_points(bbox, l.coords())),
            Kind::MultiPolygon(g) => g.polygons().try_for_each(|p| self.extend_polygon(bbox, &p)),
            Kind::GeometryCollection(g) => g
                .geometries()
                .try_for_each(|child| self.extend(bbox, &child)),
            Kind::Rect(_) | Kind::Triangle(_) => Ok(()),
        }
    }

    fn extend_polygon(
        &self,
        bbox: &mut Bbox,
        g: &impl PolygonTrait<T = f64>,
    ) -> Result<(), TwkbError> {
        for ring in g.exterior().into_iter().chain(g.interiors()) {
            self.extend_points(bbox, ring.coords())?;
        }
        Ok(())
    }

    fn extend_points<C: CoordTrait<T = f64>>(
        &self,
        bbox: &mut Bbox,
        coords: impl Iterator<Item = C>,
    ) -> Result<(), TwkbError> {
        for c in coords {
            // A geometry inside a collection may have fewer ordinates than the collection
            for d in 0..self.n.min(c.dim().size()) {
                let q = to_stored(c.nth_or_panic(d), self.digits[d])?;
                bbox.min[d] = bbox.min[d].min(q);
                bbox.max[d] = bbox.max[d].max(q);
            }
        }
        Ok(())
    }
}

/// The smallest and largest stored value of each ordinate.
struct Bbox {
    min: [i64; 4],
    max: [i64; 4],
}

#[cfg(test)]
mod tests {
    use insta::{assert_compact_debug_snapshot, assert_snapshot};

    use super::*;
    use crate::common::hex;
    use crate::reader::Twkb;

    #[test]
    fn scaled_lines_match_postgis() {
        // A way of three nodes: PostGIS's ST_AsTWKB(geom, 7) gives these bytes
        let coords = [
            [-710_366_718, 423_734_493],
            [-710_367_619, 423_733_120],
            [-710_367_827, 423_733_207],
        ];
        let mut out = Vec::new();
        write_scaled_line_string(&mut out, 7, coords.into_iter()).unwrap();
        assert_snapshot!(hex(&out), @"e20003fbd7baa505bab38d9403890eb9159f03ae01");
        let Twkb::LineString(l) = Twkb::new(&out).unwrap() else {
            unreachable!()
        };
        assert_eq!(l.stored_coords(), coords.as_flattened());
        let mut point = Vec::new();
        write_scaled_point(&mut point, 7, coords[0]).unwrap();
        assert_snapshot!(hex(&point), @"e100fbd7baa505bab38d9403");
        let mut empty = Vec::new();
        write_scaled_line_string(&mut empty, 7, [].into_iter()).unwrap();
        assert_snapshot!(hex(&empty), @"e210");
        assert_compact_debug_snapshot!(write_scaled_point(&mut point, 8, [0, 0]), @"Err(Precision(8))");
    }
}
