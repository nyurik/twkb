//! What the reader and the writer share: the header's fields, precision, varints and zigzag.

use crate::error::TwkbError;

/// The header's metadata flags.
pub(crate) const BBOX: u8 = 0b0000_0001;
pub(crate) const SIZE: u8 = 0b0000_0010;
pub(crate) const ID_LIST: u8 = 0b0000_0100;
pub(crate) const EXTENDED: u8 = 0b0000_1000;
pub(crate) const EMPTY: u8 = 0b0001_0000;

/// A geometry's type, as the low 4 bits of the TWKB header give it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GeometryType {
    /// A point.
    Point = 1,
    /// A line string.
    LineString = 2,
    /// A polygon: rings, the first the exterior.
    Polygon = 3,
    /// Points.
    MultiPoint = 4,
    /// Line strings.
    MultiLineString = 5,
    /// Polygons.
    MultiPolygon = 6,
    /// Geometries of any type, each with its own header.
    GeometryCollection = 7,
}

impl GeometryType {
    pub(crate) fn from_code(code: u8) -> Result<Self, TwkbError> {
        Ok(match code {
            1 => Self::Point,
            2 => Self::LineString,
            3 => Self::Polygon,
            4 => Self::MultiPoint,
            5 => Self::MultiLineString,
            6 => Self::MultiPolygon,
            7 => Self::GeometryCollection,
            _ => return Err(TwkbError::UnknownType(code)),
        })
    }
}

/// The ordinates of each coordinate: x and y, and z, m or both.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Dimension {
    /// x and y.
    #[default]
    Xy,
    /// x, y and z.
    Xyz,
    /// x, y and m.
    Xym,
    /// x, y, z and m.
    Xyzm,
}

impl Dimension {
    /// The number of ordinates in a coordinate, 2 to 4.
    #[must_use]
    pub fn size(self) -> usize {
        match self {
            Self::Xy => 2,
            Self::Xyz | Self::Xym => 3,
            Self::Xyzm => 4,
        }
    }

    pub(crate) fn has_z(self) -> bool {
        matches!(self, Self::Xyz | Self::Xyzm)
    }

    pub(crate) fn has_m(self) -> bool {
        matches!(self, Self::Xym | Self::Xyzm)
    }

    pub(crate) fn new(z: bool, m: bool) -> Self {
        match (z, m) {
            (false, false) => Self::Xy,
            (true, false) => Self::Xyz,
            (false, true) => Self::Xym,
            (true, true) => Self::Xyzm,
        }
    }
}

impl From<Dimension> for geo_traits::Dimensions {
    fn from(d: Dimension) -> Self {
        match d {
            Dimension::Xy => Self::Xy,
            Dimension::Xyz => Self::Xyz,
            Dimension::Xym => Self::Xym,
            Dimension::Xyzm => Self::Xyzm,
        }
    }
}

impl TryFrom<geo_traits::Dimensions> for Dimension {
    type Error = TwkbError;

    fn try_from(d: geo_traits::Dimensions) -> Result<Self, TwkbError> {
        use geo_traits::Dimensions as D;
        Ok(match d {
            D::Xy | D::Unknown(2) => Self::Xy,
            D::Xyz | D::Unknown(3) => Self::Xyz,
            D::Xym => Self::Xym,
            D::Xyzm | D::Unknown(4) => Self::Xyzm,
            D::Unknown(n) => return Err(TwkbError::Dimensions(n)),
        })
    }
}

/// The decimal digits each ordinate keeps: an ordinate `v` is stored as the integer `round(v * 10^digits)`. x and y
/// take -8 to 7 (a negative precision rounds to tens, hundreds, and so on), z and m 0 to 7.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Precision {
    /// x and y's digits, -8 to 7.
    pub xy: i8,
    /// z's digits, 0 to 7.
    pub z: u8,
    /// m's digits, 0 to 7.
    pub m: u8,
}

impl Precision {
    /// The same digits for x and y, and none for z and m.
    #[must_use]
    pub fn xy(xy: i8) -> Self {
        Self { xy, z: 0, m: 0 }
    }

    pub(crate) fn check(self) -> Result<(), TwkbError> {
        if !(-8..=7).contains(&self.xy) {
            return Err(TwkbError::Precision(self.xy.into()));
        }
        for p in [self.z, self.m] {
            if p > 7 {
                return Err(TwkbError::Precision(p.into()));
            }
        }
        Ok(())
    }

    /// The digits of each ordinate of a coordinate with `dim`, in order.
    pub(crate) fn digits(self, dim: Dimension) -> [i8; 4] {
        // z and m are at most 7, checked when read or written
        let (z, m) = (self.z.cast_signed(), self.m.cast_signed());
        match dim {
            Dimension::Xy | Dimension::Xyz => [self.xy, self.xy, z, 0],
            Dimension::Xym => [self.xy, self.xy, m, 0],
            Dimension::Xyzm => [self.xy, self.xy, z, m],
        }
    }
}

/// 10^0 to 10^8, each exact in an `f64`.
const POW10: [f64; 9] = [1.0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8];

/// The value of a stored ordinate with `digits` decimal digits.
#[inline]
#[expect(
    clippy::cast_precision_loss,
    reason = "stored ordinates are far below 2^53"
)]
pub(crate) fn to_f64(stored: i64, digits: i8) -> f64 {
    let pow = POW10[usize::from(digits.unsigned_abs())];
    if digits >= 0 {
        stored as f64 / pow
    } else {
        stored as f64 * pow
    }
}

/// An ordinate rounded to `digits` decimal digits and stored as an integer, half away from zero; an error when it is
/// not finite or does not fit an `i64` once scaled.
#[inline]
pub(crate) fn to_stored(value: f64, digits: i8) -> Result<i64, TwkbError> {
    let pow = POW10[usize::from(digits.unsigned_abs())];
    let scaled = if digits >= 0 {
        value * pow
    } else {
        value / pow
    };
    // One compare: false for NaN and the infinities, and below 2^63 the rounding cannot overflow
    if scaled.abs() < TWO_POW_63 {
        Ok(round(scaled))
    } else {
        Err(TwkbError::OutOfRange { value, digits })
    }
}

/// 2^63, exact as an `f64`.
#[expect(
    clippy::lossy_float_literal,
    reason = "2^63 is a power of two, which an f64 holds exactly"
)]
const TWO_POW_63: f64 = 9_223_372_036_854_775_808.0;

/// `v` rounded half away from zero, as `f64::round` does, without its library call on CPUs before SSE 4.1: the
/// truncating cast is one instruction, and the fraction it leaves is exact below 2^53, where an `f64` has one.
#[inline]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "the cast truncates, and the caller keeps `v` inside the range of `i64`"
)]
fn round(v: f64) -> i64 {
    let t = v as i64;
    let fraction = v - t as f64;
    if fraction >= 0.5 {
        t + 1
    } else if fraction <= -0.5 {
        t - 1
    } else {
        t
    }
}

#[inline]
pub(crate) fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)).cast_unsigned()
}

#[inline]
pub(crate) fn unzigzag(v: u64) -> i64 {
    (v >> 1).cast_signed() ^ -(v & 1).cast_signed()
}

/// Appends `v` as an unsigned varint: 7 bits per byte, low first, the high bit set on all but the last.
#[inline]
#[expect(clippy::cast_possible_truncation, reason = "the low 7 bits, by design")]
pub(crate) fn write_uvarint(out: &mut Vec<u8>, mut v: u64) {
    // Steps between coordinates mostly take one or two bytes
    if v < 0x80 {
        out.push(v as u8);
        return;
    }
    if v < 0x4000 {
        out.extend_from_slice(&[v as u8 | 0x80, (v >> 7) as u8]);
        return;
    }
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

#[inline]
pub(crate) fn write_svarint(out: &mut Vec<u8>, v: i64) {
    write_uvarint(out, zigzag(v));
}

#[cfg(test)]
pub(crate) fn hex(b: &[u8]) -> String {
    use std::fmt::Write as _;
    b.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use super::*;
    use crate::reader::Cursor;

    #[test]
    fn varints_round_trip() {
        let mut out = Vec::new();
        write_uvarint(&mut out, 200);
        assert_snapshot!(hex(&out), @"c801");
        // From twkbC's tests: 19, 200, -1, -100 and 2, each read back
        let signed = [19, 200, -1, -100, 2].map(|v| {
            let mut out = Vec::new();
            write_svarint(&mut out, v);
            assert_eq!(Cursor::new(&out, 0, 0).svarint(), Ok(v));
            format!("{v}: {}", hex(&out))
        });
        assert_snapshot!(signed.join("\n"), @"
        19: 26
        200: 9003
        -1: 01
        -100: c701
        2: 04
        ");
        for v in [0, 1, -1, i64::MIN, i64::MAX] {
            assert_eq!(unzigzag(zigzag(v)), v);
            let mut out = Vec::new();
            write_svarint(&mut out, v);
            assert_eq!(Cursor::new(&out, 0, 0).svarint(), Ok(v));
        }
    }

    #[test]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "comparing with what the cast gives"
    )]
    fn ordinates_scale() {
        let scaled = [
            format!("{:?}", to_stored(-71.036_671_8, 7)),
            format!("{}", to_f64(-710_366_718, 7)),
            format!("{:?}", to_stored(12_345_678.123_456_78, -7)),
            format!("{}", to_f64(1, -7)),
            format!("{:?}", to_stored(-2.5, 0)),
        ];
        assert_snapshot!(scaled.join(" "), @"Ok(-710366718) -71.0366718 Ok(1) 10000000 Ok(-3)");
        // Half away from zero, as f64::round, also just below a half and far beyond 2^53
        for v in [
            0.5,
            1.5,
            2.5,
            -0.5,
            0.499_999_999_999_999_94,
            -1.499_999_999_999_999_8,
            4.5e15 + 0.5,
            9.2e18,
            -9.2e18,
        ] {
            assert_eq!(round(v), v.round() as i64, "{v}");
        }
        // Not finite, or beyond i64 once scaled
        let bad = [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            1e300,
            9.3e11,
            -9.3e11,
        ]
        .map(|v| format!("{:?}", to_stored(v, 7)));
        assert_snapshot!(bad.join("\n"), @"
        Err(OutOfRange { value: NaN, digits: 7 })
        Err(OutOfRange { value: inf, digits: 7 })
        Err(OutOfRange { value: -inf, digits: 7 })
        Err(OutOfRange { value: 1e300, digits: 7 })
        Err(OutOfRange { value: 930000000000.0, digits: 7 })
        Err(OutOfRange { value: -930000000000.0, digits: 7 })
        ");
        assert_eq!(
            to_stored(-922_337_203_685.0, 7),
            Ok(-9_223_372_036_849_999_872)
        );
    }
}
