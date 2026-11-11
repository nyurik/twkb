//! The errors of reading and writing TWKB.

use std::error::Error;
use std::fmt;

/// Why a TWKB buffer could not be read, or a geometry could not be written.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum TwkbError {
    /// The buffer ends inside a geometry, at this byte.
    UnexpectedEnd(usize),
    /// A varint, starting at this byte, is longer than the 10 bytes a 64-bit value takes, or its tenth byte holds
    /// bits beyond the 64.
    VarintTooLong(usize),
    /// The header names a geometry type TWKB does not have.
    UnknownType(u8),
    /// A count of points, rings or parts that the rest of the buffer cannot hold.
    CountTooLarge {
        /// The count read.
        count: u64,
        /// Where it was read.
        at: usize,
    },
    /// The header's size differs from the size of the geometry's body.
    SizeMismatch {
        /// The size the header gives.
        expected: u64,
        /// The size of the body read.
        actual: u64,
    },
    /// Bytes follow the geometry in a buffer that should hold only it; [`Twkb::read`](crate::Twkb::read)
    /// reads a geometry from a stream instead.
    TrailingBytes(usize),
    /// Geometry collections nested deeper than [`ReadOptions::max_nesting`](crate::ReadOptions::max_nesting).
    TooDeep(usize),
    /// Decoding would allocate more than [`ReadOptions::max_bytes`](crate::ReadOptions::max_bytes).
    MemoryLimit(usize),
    /// A precision TWKB cannot hold: x and y take -8 to 7 decimal digits, z and m 0 to 7.
    Precision(i32),
    /// A geometry with dimensions TWKB cannot hold.
    Dimensions(usize),
    /// A geometry type TWKB cannot hold: rectangles and triangles, which can be written as polygons.
    Unsupported(&'static str),
    /// An ordinate that is not finite, or that does not fit an `i64` once scaled to the precision's digits.
    OutOfRange {
        /// The ordinate.
        value: f64,
        /// The decimal digits it was to be stored with.
        digits: i8,
    },
    /// An `ExactSizeIterator` yielded another number of items than its `len()`.
    LengthMismatch {
        /// The `len()` it gave, written as the count.
        len: usize,
        /// The items it yielded.
        items: usize,
    },
}

impl fmt::Display for TwkbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd(at) => write!(f, "the TWKB ends inside a geometry, at byte {at}"),
            Self::VarintTooLong(at) => write!(f, "the varint at byte {at} does not fit 64 bits"),
            Self::UnknownType(t) => write!(f, "unknown TWKB geometry type {t}"),
            Self::CountTooLarge { count, at } => {
                write!(
                    f,
                    "a count of {count} at byte {at} is more than the rest of the TWKB can hold"
                )
            }
            Self::SizeMismatch { expected, actual } => {
                write!(
                    f,
                    "the TWKB's size field says {expected} bytes, but its body has {actual}"
                )
            }
            Self::TrailingBytes(n) => write!(f, "{n} bytes follow the TWKB geometry"),
            Self::TooDeep(n) => write!(f, "geometry collections are nested deeper than {n}"),
            Self::MemoryLimit(n) => write!(
                f,
                "decoding the TWKB would take more than {n} bytes of memory"
            ),
            Self::Precision(p) => {
                write!(
                    f,
                    "a precision of {p} decimal digits is out of range: x and y take -8 to 7, z and m 0 to 7"
                )
            }
            Self::Dimensions(n) => write!(
                f,
                "{n} dimensions cannot be written as TWKB, which has 2 to 4"
            ),
            Self::Unsupported(kind) => write!(
                f,
                "a {kind} cannot be written as TWKB: write it as a polygon"
            ),
            Self::OutOfRange { value, digits } => {
                write!(
                    f,
                    "the ordinate {value} cannot be stored with {digits} decimal digits in 64 bits"
                )
            }
            Self::LengthMismatch { len, items } => {
                write!(f, "an iterator of length {len} yielded {items} items")
            }
        }
    }
}

impl Error for TwkbError {}
