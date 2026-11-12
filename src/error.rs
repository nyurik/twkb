//! The errors of reading and writing TWKB.

/// Why a TWKB buffer could not be read, or a geometry could not be written.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum TwkbError {
    /// The buffer ends inside a geometry, at this byte.
    #[error("the TWKB ends inside a geometry, at byte {0}")]
    UnexpectedEnd(usize),
    /// A varint, starting at this byte, is longer than the 10 bytes a 64-bit value takes, or its tenth byte holds
    /// bits beyond the 64.
    #[error("the varint at byte {0} does not fit 64 bits")]
    VarintTooLong(usize),
    /// The header names a geometry type TWKB does not have.
    #[error("unknown TWKB geometry type {0}")]
    UnknownType(u8),
    /// A count of points, rings or parts that the rest of the buffer cannot hold.
    #[error("a count of {count} at byte {at} is more than the rest of the TWKB can hold")]
    CountTooLarge {
        /// The count read.
        count: u64,
        /// Where it was read.
        at: usize,
    },
    /// The header's size differs from the size of the geometry's body.
    #[error("the TWKB's size field says {expected} bytes, but its body has {actual}")]
    SizeMismatch {
        /// The size the header gives.
        expected: u64,
        /// The size of the body read.
        actual: u64,
    },
    /// Bytes follow the geometry in a buffer that should hold only it; [`Twkb::read`](crate::Twkb::read)
    /// reads a geometry from a stream instead.
    #[error("{0} bytes follow the TWKB geometry")]
    TrailingBytes(usize),
    /// Geometry collections nested deeper than [`ReadOptions::max_nesting`](crate::ReadOptions::max_nesting).
    #[error("geometry collections are nested deeper than {0}")]
    TooDeep(usize),
    /// Decoding would allocate more than [`ReadOptions::max_bytes`](crate::ReadOptions::max_bytes).
    #[error("decoding the TWKB would take more than {0} bytes of memory")]
    MemoryLimit(usize),
    /// A precision TWKB cannot hold: x and y take -8 to 7 decimal digits, z and m 0 to 7.
    #[error(
        "a precision of {0} decimal digits is out of range: x and y take -8 to 7, z and m 0 to 7"
    )]
    Precision(i32),
    /// A geometry with dimensions TWKB cannot hold.
    #[error("{0} dimensions cannot be written as TWKB, which has 2 to 4")]
    Dimensions(usize),
    /// A geometry type TWKB cannot hold: rectangles and triangles, which can be written as polygons.
    #[error("a {0} cannot be written as TWKB: write it as a polygon")]
    Unsupported(&'static str),
    /// An ordinate that is not finite, or that does not fit an `i64` once scaled to the precision's digits.
    #[error("the ordinate {value} cannot be stored with {digits} decimal digits in 64 bits")]
    OutOfRange {
        /// The ordinate.
        value: f64,
        /// The decimal digits it was to be stored with.
        digits: i8,
    },
    /// An `ExactSizeIterator` yielded another number of items than its `len()`.
    #[error("an iterator of length {len} yielded {items} items")]
    LengthMismatch {
        /// The `len()` it gave, written as the count.
        len: usize,
        /// The items it yielded.
        items: usize,
    },
}
