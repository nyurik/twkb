//! Hex, for the examples' input and output.

use std::fmt::Write as _;

/// The bytes of `s`, if it is hex.
pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.is_empty() || !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

/// `b` as lowercase hex.
pub fn to_hex(b: &[u8]) -> String {
    b.iter()
        .fold(String::with_capacity(b.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}
