//! Reading varints from a TWKB buffer, within a memory budget.

use crate::common::unzigzag;
use crate::error::TwkbError;

/// The unsigned varint at `pos`, and the position after it; `None` if it is cut short, longer than 10 bytes, or holds
/// bits beyond 64. The one and two byte varints most steps between coordinates take are read without a loop.
#[inline(always)]
#[expect(
    clippy::inline_always,
    reason = "measured: reading a line of 1,000 points took 14.7 µs inlined as the compiler chose, 5.8 µs always"
)]
pub(crate) fn uvarint_at(buf: &[u8], pos: usize) -> Option<(u64, usize)> {
    let b0 = *buf.get(pos)?;
    if b0 < 0x80 {
        return Some((u64::from(b0), pos + 1));
    }
    let b1 = *buf.get(pos + 1)?;
    let mut v = u64::from(b0 & 0x7f) | (u64::from(b1 & 0x7f) << 7);
    if b1 < 0x80 {
        return Some((v, pos + 2));
    }
    let mut pos = pos + 2;
    let mut shift = 14;
    while shift < 63 {
        let b = *buf.get(pos)?;
        pos += 1;
        v |= u64::from(b & 0x7f) << shift;
        if b < 0x80 {
            return Some((v, pos));
        }
        shift += 7;
    }
    // The tenth byte holds the 64th bit alone
    let b = *buf.get(pos)?;
    (b <= 1).then(|| (v | (u64::from(b) << 63), pos + 1))
}

/// Why [`uvarint_at`] could not read the varint at `pos`.
#[cold]
pub(crate) fn varint_error(buf: &[u8], pos: usize) -> TwkbError {
    let rest = buf.get(pos..).unwrap_or_default();
    if rest.len() < 10 && rest.iter().all(|&b| b >= 0x80) {
        TwkbError::UnexpectedEnd(buf.len())
    } else {
        TwkbError::VarintTooLong(pos)
    }
}

/// A position in a TWKB buffer, and the bytes decoding may still allocate.
pub(crate) struct Cursor<'a> {
    pub(crate) buf: &'a [u8],
    pub(crate) pos: usize,
    budget: usize,
    max_bytes: usize,
    pub(crate) max_nesting: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(buf: &'a [u8], max_bytes: usize, max_nesting: usize) -> Self {
        Self {
            buf,
            pos: 0,
            budget: max_bytes,
            max_bytes,
            max_nesting,
        }
    }

    /// Takes the memory of `count` values of `T` from the budget, before they are allocated.
    #[inline]
    pub(crate) fn charge<T>(&mut self, count: usize) -> Result<(), TwkbError> {
        match count.checked_mul(size_of::<T>()) {
            Some(bytes) if bytes <= self.budget => {
                self.budget -= bytes;
                Ok(())
            }
            _ => Err(TwkbError::MemoryLimit(self.max_bytes)),
        }
    }

    #[inline]
    pub(crate) fn u8(&mut self) -> Result<u8, TwkbError> {
        let b = *self
            .buf
            .get(self.pos)
            .ok_or(TwkbError::UnexpectedEnd(self.pos))?;
        self.pos += 1;
        Ok(b)
    }

    #[inline]
    pub(crate) fn uvarint(&mut self) -> Result<u64, TwkbError> {
        let (v, next) =
            uvarint_at(self.buf, self.pos).ok_or_else(|| varint_error(self.buf, self.pos))?;
        self.pos = next;
        Ok(v)
    }

    #[inline]
    pub(crate) fn svarint(&mut self) -> Result<i64, TwkbError> {
        self.uvarint().map(unzigzag)
    }

    /// A count of items, each taking at least `min_bytes` bytes, which the rest of the buffer must be able to hold.
    pub(crate) fn count(&mut self, min_bytes: usize) -> Result<usize, TwkbError> {
        let at = self.pos;
        let count = self.uvarint()?;
        let rest = (self.buf.len() - self.pos) as u64;
        match usize::try_from(count) {
            Ok(n) if count.saturating_mul(min_bytes as u64) <= rest => Ok(n),
            _ => Err(TwkbError::CountTooLarge { count, at }),
        }
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_compact_debug_snapshot;

    use super::*;

    #[test]
    fn rejects_bad_varints() {
        let read = |b: &[u8]| Cursor::new(b, 0, 0).uvarint();
        let mut max = [0xff; 10];
        max[9] = 0x01;
        assert_eq!(read(&max), Ok(u64::MAX));
        // A tenth byte with more than the 64th bit
        max[9] = 0x02;
        assert_compact_debug_snapshot!(read(&max), @"Err(VarintTooLong(0))");
        assert_compact_debug_snapshot!(read(&[0xff; 11]), @"Err(VarintTooLong(0))");
        assert_compact_debug_snapshot!(read(&[0x80]), @"Err(UnexpectedEnd(1))");
        assert_compact_debug_snapshot!(read(&[0xff; 9]), @"Err(UnexpectedEnd(9))");
    }
}
