//! Reading TWKB into geometries that implement [geo-traits](geo_traits).
//!
//! A TWKB coordinate is a step from the one before, so nothing can be read in place: [`Twkb::new`] decodes a buffer
//! once into flat vectors of the stored integers, and the geometries read their `f64` ordinates from them, scaled by
//! the header's precision. The integers themselves are there too, for sources that keep integer coordinates.

mod cursor;
mod geometry;

pub(crate) use self::cursor::Cursor;
use self::cursor::{uvarint_at, varint_error};
pub use self::geometry::{
    Coord, GeometryCollection, LineString, LineStringRef, MultiLineString, MultiPoint,
    MultiPolygon, Point, PointRef, Polygon, PolygonRef, Twkb,
};
use crate::common::{
    BBOX, Dimension, EMPTY, EXTENDED, GeometryType, ID_LIST, Precision, SIZE, to_f64, unzigzag,
};
use crate::error::TwkbError;

/// The limits of decoding, so that a bad or hostile buffer is an error rather than a crash.
///
/// Every count in the buffer is checked against the bytes after it, so coordinates take at most 8 bytes of memory per
/// byte of TWKB, and the geometries of a collection, of which each takes 2 bytes or more, at most 64 (a decoded
/// geometry is 128 bytes on 64-bit targets). `max_bytes` caps it further.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadOptions {
    /// The most memory a geometry's coordinates, parts and IDs may take, in bytes: 256 MiB by default.
    pub max_bytes: usize,
    /// How deep geometry collections may nest: each level is a call, so an unbounded depth would let a short buffer
    /// overflow the stack. 128 by default.
    pub max_nesting: usize,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            max_bytes: 256 << 20,
            max_nesting: 128,
        }
    }
}

/// A geometry's header: its type, dimensions and precision, and the optional size, bounding box and ID list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    kind: GeometryType,
    dim: Dimension,
    precision: Precision,
    /// The digits of each ordinate, in order.
    pub(crate) digits: [i8; 4],
    size: Option<u64>,
    /// Boxed, as most geometries have none: a box inline would double the size of a decoded point
    bbox: Option<Box<[i64; 8]>>,
    ids: Option<Box<[i64]>>,
    empty: bool,
}

impl Header {
    /// The geometry's type.
    #[must_use]
    pub fn geometry_type(&self) -> GeometryType {
        self.kind
    }

    /// The ordinates of each coordinate.
    #[must_use]
    pub fn dimension(&self) -> Dimension {
        self.dim
    }

    /// The decimal digits each ordinate keeps.
    #[must_use]
    pub fn precision(&self) -> Precision {
        self.precision
    }

    /// The size of the rest of the geometry in bytes, if the header gives it.
    #[must_use]
    pub fn size(&self) -> Option<u64> {
        self.size
    }

    /// The bounding box, if the header gives it, as stored: each ordinate's minimum and maximum, x first, as
    /// integers at the [precision](Self::precision).
    #[must_use]
    pub fn bbox_stored(&self) -> Option<&[i64]> {
        let n = self.dim.size();
        self.bbox.as_ref().map(|b| &b[..2 * n])
    }

    /// The bounding box, if the header gives it: each ordinate's minimum and maximum, x first.
    #[must_use]
    pub fn bbox(&self) -> Option<impl Iterator<Item = (f64, f64)> + '_> {
        let digits = self.digits;
        self.bbox_stored().map(move |b| {
            b.as_chunks::<2>()
                .0
                .iter()
                .zip(digits)
                .map(|(&[min, max], d)| (to_f64(min, d), to_f64(max, d)))
        })
    }

    /// The IDs of a multi-geometry's or a collection's parts, if the header gives them.
    #[must_use]
    pub fn ids(&self) -> Option<&[i64]> {
        self.ids.as_deref()
    }

    /// Whether the geometry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.empty
    }
}

impl Twkb {
    /// Reads the one geometry `buf` holds, with the default [`ReadOptions`]; bytes after it are an error.
    pub fn new(buf: &[u8]) -> Result<Self, TwkbError> {
        Self::new_with(buf, &ReadOptions::default())
    }

    /// Reads the one geometry `buf` holds; bytes after it are an error.
    pub fn new_with(buf: &[u8], opts: &ReadOptions) -> Result<Self, TwkbError> {
        let mut rest = buf;
        let geometry = Self::read_with(&mut rest, opts)?;
        if rest.is_empty() {
            Ok(geometry)
        } else {
            Err(TwkbError::TrailingBytes(rest.len()))
        }
    }

    /// Reads the geometry at the start of `buf` with the default [`ReadOptions`], and moves `buf` past it: for a
    /// stream of geometries, one after another.
    pub fn read(buf: &mut &[u8]) -> Result<Self, TwkbError> {
        Self::read_with(buf, &ReadOptions::default())
    }

    /// Reads the geometry at the start of `buf`, and moves `buf` past it.
    pub fn read_with(buf: &mut &[u8], opts: &ReadOptions) -> Result<Self, TwkbError> {
        let mut c = Cursor::new(buf, opts.max_bytes, opts.max_nesting);
        let geometry = read_geometry(&mut c, opts.max_nesting)?;
        *buf = &buf[c.pos..];
        Ok(geometry)
    }
}

/// Reads a geometry, header and body, at the cursor, with collections nested at most `nesting` deep inside it.
fn read_geometry(c: &mut Cursor<'_>, nesting: usize) -> Result<Twkb, TwkbError> {
    let first = c.u8()?;
    let kind = GeometryType::from_code(first & 0x0f)?;
    // A zigzag value of 4 bits, -8 to 7
    #[expect(
        clippy::cast_possible_truncation,
        reason = "4 bits unzigzagged are -8 to 7"
    )]
    let xy = unzigzag(u64::from(first >> 4)) as i8;
    let flags = c.u8()?;
    let (dim, z, m) = if flags & EXTENDED == 0 {
        (Dimension::Xy, 0, 0)
    } else {
        let e = c.u8()?;
        (Dimension::new(e & 1 != 0, e & 2 != 0), (e >> 2) & 7, e >> 5)
    };
    let precision = Precision { xy, z, m };
    let size = if flags & SIZE == 0 {
        None
    } else {
        Some(c.uvarint()?)
    };
    let start = c.pos;
    let mut header = Header {
        kind,
        dim,
        precision,
        digits: precision.digits(dim),
        size,
        bbox: None,
        ids: None,
        empty: flags & EMPTY != 0,
    };
    let n = dim.size();
    let mut body = Body::default();
    if !header.empty {
        if flags & BBOX != 0 {
            header.bbox = Some(read_bbox(c, n)?);
        }
        let mut coords = Coords::new(n);
        match kind {
            GeometryType::Point => coords.read(c, 1)?,
            GeometryType::LineString => {
                let points = c.count(n)?;
                coords.read(c, points)?;
            }
            GeometryType::Polygon => body.rings = coords.read_rings(c)?,
            GeometryType::MultiPoint => {
                let points = c.count(n)?;
                header.ids = read_ids(c, flags, points)?;
                coords.read(c, points)?;
            }
            GeometryType::MultiLineString => {
                let lines = c.count(1)?;
                header.ids = read_ids(c, flags, lines)?;
                c.charge::<usize>(lines)?;
                body.parts.reserve(lines);
                for _ in 0..lines {
                    let points = c.count(n)?;
                    coords.read(c, points)?;
                    body.parts.push(coords.len());
                }
            }
            GeometryType::MultiPolygon => {
                let polygons = c.count(1)?;
                header.ids = read_ids(c, flags, polygons)?;
                c.charge::<usize>(polygons)?;
                body.parts.reserve(polygons);
                for _ in 0..polygons {
                    let rings = coords.read_rings(c)?;
                    body.rings.extend(rings);
                    body.parts.push(body.rings.len());
                }
            }
            GeometryType::GeometryCollection => {
                let Some(inner) = nesting.checked_sub(1) else {
                    return Err(TwkbError::TooDeep(c.max_nesting));
                };
                let count = c.count(2)?;
                header.ids = read_ids(c, flags, count)?;
                c.charge::<Twkb>(count)?;
                body.geometries.reserve(count);
                for _ in 0..count {
                    body.geometries.push(read_geometry(c, inner)?);
                }
            }
        }
        body.coords = coords.values;
    }
    if let Some(expected) = size {
        let actual = (c.pos - start) as u64;
        if actual != expected {
            return Err(TwkbError::SizeMismatch { expected, actual });
        }
    }
    Ok(Twkb::from_parts(header, body))
}

/// A bounding box of `n` dimensions: each ordinate's minimum, then the step to its maximum.
fn read_bbox(c: &mut Cursor<'_>, n: usize) -> Result<Box<[i64; 8]>, TwkbError> {
    c.charge::<[i64; 8]>(1)?;
    let mut bbox = Box::new([0; 8]);
    for d in 0..n {
        let min = c.svarint()?;
        bbox[2 * d] = min;
        bbox[2 * d + 1] = min.wrapping_add(c.svarint()?);
    }
    Ok(bbox)
}

/// The IDs of `count` parts, if the flags say they follow.
fn read_ids(c: &mut Cursor<'_>, flags: u8, count: usize) -> Result<Option<Box<[i64]>>, TwkbError> {
    if flags & ID_LIST == 0 {
        return Ok(None);
    }
    c.charge::<i64>(count)?;
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        ids.push(c.svarint()?);
    }
    Ok(Some(ids.into_boxed_slice()))
}

/// What a geometry's body holds, by type: the stored ordinates, and where each part ends.
#[derive(Default)]
pub(crate) struct Body {
    pub(crate) coords: Vec<i64>,
    /// The end of each ring, in coordinates.
    pub(crate) rings: Vec<usize>,
    /// The end of each line string, in coordinates, or of each polygon, in rings.
    pub(crate) parts: Vec<usize>,
    pub(crate) geometries: Vec<Twkb>,
}

/// The ordinates of a geometry's coordinates as they are read: each a step from the one before, across all its
/// parts.
struct Coords {
    values: Vec<i64>,
    last: [i64; 4],
    n: usize,
}

impl Coords {
    fn new(n: usize) -> Self {
        Self {
            values: Vec::new(),
            last: [0; 4],
            n,
        }
    }

    /// The number of coordinates read.
    fn len(&self) -> usize {
        self.values.len() / self.n
    }

    fn read(&mut self, c: &mut Cursor<'_>, points: usize) -> Result<(), TwkbError> {
        c.charge::<i64>(points * self.n)?;
        self.values.reserve(points * self.n);
        if self.n == 2 {
            // x and y alone, the common case: one loop over the buffer
            let [mut x, mut y, ..] = self.last;
            let mut pos = c.pos;
            for _ in 0..points {
                let Some((dx, p)) = uvarint_at(c.buf, pos) else {
                    return Err(varint_error(c.buf, pos));
                };
                let Some((dy, p)) = uvarint_at(c.buf, p) else {
                    return Err(varint_error(c.buf, p));
                };
                pos = p;
                x = x.wrapping_add(unzigzag(dx));
                y = y.wrapping_add(unzigzag(dy));
                self.values.extend([x, y]);
            }
            c.pos = pos;
            self.last[..2].copy_from_slice(&[x, y]);
            return Ok(());
        }
        for _ in 0..points {
            for d in 0..self.n {
                self.last[d] = self.last[d].wrapping_add(c.svarint()?);
                self.values.push(self.last[d]);
            }
        }
        Ok(())
    }

    /// A polygon's rings, and the end of each, in coordinates.
    fn read_rings(&mut self, c: &mut Cursor<'_>) -> Result<Vec<usize>, TwkbError> {
        let count = c.count(1)?;
        c.charge::<usize>(count)?;
        let mut ends = Vec::with_capacity(count);
        for _ in 0..count {
            let points = c.count(self.n)?;
            self.read(c, points)?;
            ends.push(self.len());
        }
        Ok(ends)
    }
}

#[cfg(test)]
mod tests {
    use geo_traits::{CoordTrait as _, GeometryTrait as _, LineStringTrait as _, PointTrait as _};
    use insta::assert_compact_debug_snapshot;

    use super::*;

    #[test]
    fn reads_points_and_lines() {
        // POINT(1 2), from twkb.js's tests
        let Twkb::Point(p) = Twkb::new(&[0x01, 0x00, 0x02, 0x04]).unwrap() else {
            unreachable!()
        };
        assert_compact_debug_snapshot!(p.coord().map(|c| c.x_y()), @"Some((1.0, 2.0))");
        assert_compact_debug_snapshot!(p.stored_coords(), @"[1, 2]");
        // LINESTRING(1 1,5 5) with its size and box
        let g = Twkb::new(&[
            0x02, 0x03, 0x09, 0x02, 0x08, 0x02, 0x08, 0x02, 0x02, 0x02, 0x08, 0x08,
        ])
        .unwrap();
        assert_compact_debug_snapshot!((g.header().size(), g.header().bbox_stored(), g.dim()), @"(Some(9), Some([1, 5, 1, 5]), Xy)");
        let Twkb::LineString(l) = &g else {
            unreachable!()
        };
        assert_compact_debug_snapshot!(l.coords().map(|c| c.x_y()).collect::<Vec<_>>(), @"[(1.0, 1.0), (5.0, 5.0)]");
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn decoded_size() {
        // What a collection's child costs in memory, charged against `ReadOptions::max_bytes`
        assert_compact_debug_snapshot!((size_of::<Header>(), size_of::<Twkb>()), @"(56, 128)");
    }

    #[test]
    fn rejects_bad_buffers() {
        // Cut short, an unknown type, a byte after the geometry
        assert_compact_debug_snapshot!(Twkb::new(&[0x01]), @"Err(UnexpectedEnd(1))");
        assert_compact_debug_snapshot!(Twkb::new(&[0x08, 0x00]), @"Err(UnknownType(8))");
        assert_compact_debug_snapshot!(Twkb::new(&[0x01, 0x00, 0x02, 0x04, 0x00]), @"Err(TrailingBytes(1))");
        // A line string of 2^60 points in a buffer of a few bytes
        assert_compact_debug_snapshot!(Twkb::new(&[0x02, 0x00, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x10]), @"Err(CountTooLarge { count: 1152921504606846976, at: 2 })");
        // A size that does not match the body
        assert_compact_debug_snapshot!(Twkb::new(&[0x01, 0x02, 0x03, 0x02, 0x04]), @"Err(SizeMismatch { expected: 3, actual: 2 })");
        // Collections inside collections, deeper than allowed
        assert_compact_debug_snapshot!(Twkb::new(&[0x07, 0x00, 0x01].repeat(129)), @"Err(TooDeep(128))");
        // A line string of 1000 points, within the bytes but not the memory allowed
        let mut long = vec![0x02, 0x00, 0xe8, 0x07];
        long.extend([0; 2000]);
        Twkb::new(&long).unwrap();
        let opts = ReadOptions {
            max_bytes: 1000,
            ..ReadOptions::default()
        };
        assert_compact_debug_snapshot!(Twkb::new_with(&long, &opts), @"Err(MemoryLimit(1000))");
        // Two geometries in a stream
        let mut stream: &[u8] = &[0x01, 0x00, 0x02, 0x04, 0x01, 0x00, 0x00, 0x00];
        Twkb::read(&mut stream).unwrap();
        assert_compact_debug_snapshot!(stream, @"[1, 0, 0, 0]");
    }
}
