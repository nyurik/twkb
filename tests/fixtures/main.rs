//! TWKB test cases from other implementations, one module per source beside its license and a README linking the
//! files the cases were taken from. Each case gives what is written and what is read back as inline snapshots: the
//! TWKB hex as the source's tests have it, and the geometry the TWKB reads as, in WKT. Every geometry read is also
//! written again with its header's options and must give the same bytes.
//!
//! Each source's case functions take a [`Runner`]: its `#[test]`s run them with [`Runner::Local`], which checks the
//! snapshots, and [`against_postgis`] collects every case with [`Runner::Collect`] and checks them all against a
//! PostGIS in one batch, with random geometries beside them.
#![allow(clippy::unwrap_used, reason = "a test fails on any error")]
#![allow(clippy::too_many_lines, reason = "a source's cases, one after another")]

#[path = "../common/mod.rs"]
mod common;

mod against_postgis;
mod devork;
mod jts;
mod own;
mod postgis;
mod twkb_c;
mod twkb_js;
mod twkbpy;
mod wkx;

use std::fmt::Write as _;
use std::str::FromStr as _;
use std::{env, fs};

use common::{hex, to_hex, wkt};
use twkb::{Precision, Twkb, WriteOptions, write_geometry};
use wkt::Wkt;

/// What runs a source's cases.
pub(crate) enum Runner {
    /// Checks each case's snapshot here.
    Local,
    /// Collects the cases, to check them elsewhere.
    Collect(Vec<Case>),
}

/// A case as data.
pub(crate) struct Case {
    /// Where the case is, as `file:line`.
    pub(crate) at: &'static str,
    /// What is written, for a case that writes.
    pub(crate) written: Option<Written>,
    /// The TWKB the case gives, as hex: the input of a case that reads, the snapshot's first line of one that writes.
    pub(crate) twkb: &'static str,
}

/// The geometry a case writes, and its options.
pub(crate) struct Written {
    pub(crate) wkt: &'static str,
    pub(crate) xy: i8,
    pub(crate) z: u8,
    pub(crate) m: u8,
    pub(crate) size: bool,
    pub(crate) bbox: bool,
    /// Whether the source's TWKB leaves out repeated points, as PostGIS does.
    pub(crate) removes_repeated: bool,
}

/// One case, run by `r`, a [`Runner`].
///
/// `case!(r, wkt, xy, z, m, size, bbox, @"twkb\nread")`: `wkt` written with those options gives `twkb`, which reads
/// as `read`, both in one snapshot. `case!(r, twkb, @"read")`: `twkb` reads as `read`.
macro_rules! case {
    ($r:ident, $wkt:literal, $xy:literal, $z:literal, $m:literal, $size:literal, $bbox:literal, @$expected:literal $(,)?) => {
        match $r {
            $crate::Runner::Local => {
                let twkb = $crate::encode($wkt, $xy, $z, $m, $size, $bbox, REMOVES_REPEATED);
                insta::assert_snapshot!(format!("{twkb}\n{}", $crate::decode(&twkb)), @$expected);
            }
            $crate::Runner::Collect(cases) => cases.push($crate::Case {
                at: concat!(file!(), ":", line!()),
                written: Some($crate::Written {
                    wkt: $wkt,
                    xy: $xy,
                    z: $z,
                    m: $m,
                    size: $size,
                    bbox: $bbox,
                    removes_repeated: REMOVES_REPEATED,
                }),
                twkb: $crate::first_line($expected),
            }),
        }
    };
    ($r:ident, $twkb:literal, @$read:literal $(,)?) => {
        match $r {
            $crate::Runner::Local => insta::assert_snapshot!($crate::decode($twkb), @$read),
            $crate::Runner::Collect(cases) => cases.push($crate::Case {
                at: concat!(file!(), ":", line!()),
                written: None,
                twkb: $twkb,
            }),
        }
    };
}
pub(crate) use case;

/// A source's case functions: `ALL` lists them, and a `#[test]` of each in `local` runs it with [`Runner::Local`].
macro_rules! cases {
    ($($f:ident),* $(,)?) => {
        /// The source's case functions.
        pub(crate) const ALL: &[fn(&mut $crate::Runner)] = &[$($f),*];

        mod local {
            $(
                #[test]
                fn $f() {
                    super::$f(&mut $crate::Runner::Local);
                }
            )*
        }
    };
}
pub(crate) use cases;

/// The case functions of every source.
pub(crate) const SOURCES: [&[fn(&mut Runner)]; 8] = [
    devork::ALL,
    jts::ALL,
    own::ALL,
    postgis::ALL,
    twkb_c::ALL,
    twkb_js::ALL,
    twkbpy::ALL,
    wkx::ALL,
];

/// Every case of every source.
pub(crate) fn all_cases() -> Vec<Case> {
    let mut runner = Runner::Collect(Vec::new());
    for source in SOURCES {
        for f in source {
            f(&mut runner);
        }
    }
    let Runner::Collect(cases) = runner else {
        unreachable!()
    };
    cases
}

/// The first line of a snapshot that is not blank, trimmed.
pub(crate) fn first_line(snapshot: &'static str) -> &'static str {
    snapshot
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
}

/// `wkt` written with these options, as hex.
pub(crate) fn encode(
    wkt: &str,
    xy: i8,
    z: u8,
    m: u8,
    size: bool,
    bbox: bool,
    remove_repeated_points: bool,
) -> String {
    let geometry = Wkt::<f64>::from_str(wkt).unwrap();
    let opts = WriteOptions {
        precision: Precision { xy, z, m },
        size,
        bbox,
        remove_repeated_points,
    };
    let mut out = Vec::new();
    write_geometry(&mut out, &geometry, &opts).unwrap();
    to_hex(&out)
}

/// The geometries of hex TWKB, one after another, each as WKT with its IDs if it has them.
///
/// Each is also written again with the options its header gives, which must give its bytes; a geometry with IDs is not,
/// as the writer writes none.
fn decode(twkb: &str) -> String {
    let bytes = hex(twkb);
    let mut rest = &bytes[..];
    let mut out = Vec::new();
    while !rest.is_empty() {
        let before = rest;
        let read = Twkb::read(&mut rest).unwrap();
        let mut text = wkt(&read);
        if let Some(ids) = read.header().ids() {
            write!(text, " ids {ids:?}").unwrap();
        } else {
            let written = &before[..before.len() - rest.len()];
            assert_eq!(
                to_hex(&write_again(&read)),
                to_hex(written),
                "{text} written again"
            );
        }
        out.push(text);
    }
    out.join("; ")
}

/// A geometry written with the options its header gives.
fn write_again(g: &Twkb) -> Vec<u8> {
    let header = g.header();
    let opts = WriteOptions {
        precision: header.precision(),
        size: header.size().is_some(),
        bbox: header.bbox_stored().is_some() || child_bbox(g),
        remove_repeated_points: false,
    };
    let mut out = Vec::new();
    write_geometry(&mut out, g, &opts).unwrap();
    out
}

/// Whether a collection's geometries have bounding boxes, though the collection, being empty, has none.
fn child_bbox(g: &Twkb) -> bool {
    use geo_traits::GeometryCollectionTrait as _;
    let Twkb::GeometryCollection(c) = g else {
        return false;
    };
    c.geometries()
        .any(|g| g.header().bbox_stored().is_some() || child_bbox(g))
}

/// Writes the TWKB of every case into the directory `TWKB_CORPUS_DIR`, one file each: the fuzz targets' starting
/// corpus (`just fuzz`).
#[test]
#[ignore = "run by `just fuzz`"]
fn write_fuzz_corpus() {
    let dir = env::var("TWKB_CORPUS_DIR").unwrap();
    fs::create_dir_all(&dir).unwrap();
    for (n, case) in all_cases().iter().enumerate() {
        fs::write(format!("{dir}/case-{n:04}"), hex(case.twkb)).unwrap();
    }
}
