//! Every case of every source, and random geometries, checked against PostGIS in one batch. Ignored by default:
//! `just test-postgis` starts PostGIS in Docker and runs it, and `just test-postgis-at <url>` runs it against the
//! PostgreSQL with PostGIS at a connection string, which it gives as `TWKB_POSTGIS_URL`.
//!
//! * A case that writes: PostGIS's [`ST_AsTWKB`](https://postgis.net/docs/ST_AsTWKB.html) of its WKT, with its options,
//!   gives its snapshot's TWKB.
//! * Every TWKB of the cases, and every one PostGIS writes from a random geometry: PostGIS's
//!   [`ST_GeomFromTWKB`](https://postgis.net/docs/ST_GeomFromTWKB.html) reads it as the geometry this crate reads,
//!   compared as WKB.
//! * A random geometry, generated as WKB with a fixed seed: this crate writes the same bytes as `ST_AsTWKB`.
//!
//! Where PostGIS differs on purpose, the case is listed in [`KNOWN`] with the reason, and must still differ.

use std::env;
use std::fmt::Write as _;

use postgres::{Client, NoTls};
use twkb::{Precision, Twkb, WriteOptions, write_geometry};
use wkb::reader::read_wkb;
use wkb::writer::{WriteOptions as WkbOptions, write_geometry as write_wkb};

use crate::common::{hex, same, shape, to_hex};
use crate::{Case, all_cases};

/// The cases PostGIS differs on, by the TWKB the case gives, and why.
///
/// Below 0 digits, PostGIS writes x and y as if scaled by a single-precision 10^precision, which is off by up to
/// about 5e-8 of the scaled value (0.01 is 0.009999999776), and so rounds wrongly within that distance of a half: 150
/// at -2 digits is 100, and 3570264.95 at -1 is 3570270. This crate scales by the exact power of 10, and rounds half
/// away from zero as at every other precision. The random geometries leave out the band where PostGIS rounds wrongly.
///
/// PostGIS also reads them divided by an `f64` 10^-precision, which is not exact either (123 at -5 digits is
/// 12299999.999999998, where this crate reads 12300000): below 0 digits, what it reads is compared within that
/// rounding rather than byte for byte.
const KNOWN: &[(&str, &str)] = &[
    (
        "31000403",
        "PostGIS writes 150 and -150 at -2 digits as 100 and -100",
    ),
    (
        "1100c4ca2b00",
        "PostGIS writes 3570264.95 at -1 digit as 3570270",
    ),
];

/// How many random geometries are checked, and the seed they come from.
const RANDOM: usize = 20_000;
const SEED: u64 = 0x7477_6b62;

/// Wraps PostGIS's functions so that an error is a row's result, not the batch's end.
const FUNCTIONS: &str = "
CREATE FUNCTION pg_temp.twkb_of_wkt(w text, xy int, z int, m int, s bool, b bool) RETURNS text AS $$
BEGIN RETURN encode(ST_AsTWKB(ST_GeomFromText(w), xy, z, m, s, b), 'hex');
EXCEPTION WHEN OTHERS THEN RETURN 'error: ' || SQLERRM; END $$ LANGUAGE plpgsql;
CREATE FUNCTION pg_temp.twkb_of_wkb(w bytea, xy int, z int, m int, s bool, b bool) RETURNS text AS $$
BEGIN RETURN encode(ST_AsTWKB(ST_GeomFromWKB(w), xy, z, m, s, b), 'hex');
EXCEPTION WHEN OTHERS THEN RETURN 'error: ' || SQLERRM; END $$ LANGUAGE plpgsql;
CREATE FUNCTION pg_temp.wkb_of_twkb(t bytea) RETURNS text AS $$
BEGIN RETURN encode(ST_AsBinary(ST_GeomFromTWKB(t), 'NDR'), 'hex');
EXCEPTION WHEN OTHERS THEN RETURN 'error: ' || SQLERRM; END $$ LANGUAGE plpgsql;
";

#[test]
#[ignore = "needs PostGIS: run it with `just test-postgis`"]
fn against_postgis() {
    let url = env::var("TWKB_POSTGIS_URL")
        .expect("TWKB_POSTGIS_URL names the PostGIS to check against: run `just test-postgis`");
    let mut db = Client::connect(&url, NoTls).unwrap();
    db.batch_execute(FUNCTIONS).unwrap();

    let cases = all_cases();

    let mut report = Report::default();
    let mut reads = Vec::new();
    check_written(&mut db, &cases, &mut report);
    for case in &cases {
        for g in geometries(&hex(case.twkb)) {
            reads.push((case.at.to_owned(), g));
        }
    }
    let random = check_random(&mut db, &mut report);
    reads.extend(random);
    check_read(&mut db, &reads, &mut report);
    report.finish(&cases);
}

/// What differs from PostGIS, by case.
#[derive(Default)]
struct Report {
    /// Each difference: the TWKB of its case, where the case is, and what differs.
    differences: Vec<(String, String, String)>,
    random: usize,
}

/// Whether PostGIS's reading of a TWKB below 0 digits differs from this crate's only as its division by an `f64`
/// 10^-precision does: an inexact divisor and a rounded quotient, together within 3.4e-16 of each value.
fn within_f64_division(twkb: &[u8], ours: &[u8], postgis: &str) -> bool {
    if Twkb::new(twkb).unwrap().header().precision().xy >= 0 {
        return false;
    }
    let postgis = hex(postgis);
    let Ok(theirs) = read_wkb(&postgis) else {
        return false;
    };
    same(&shape(&read_wkb(ours).unwrap()), &shape(&theirs), &|v| {
        3.4e-16 * v.abs()
    })
}

impl Report {
    fn differ(&mut self, twkb: &str, at: &str, what: String) {
        self.differences
            .push((twkb.to_owned(), at.to_owned(), what));
    }

    /// Fails on a difference not in [`KNOWN`], and on a known one that no longer differs.
    #[allow(clippy::print_stderr, reason = "says what was checked")]
    fn finish(self, cases: &[Case]) {
        let mut failures = String::new();
        for (twkb, at, what) in &self.differences {
            if !KNOWN.iter().any(|(k, _)| k == twkb) {
                writeln!(failures, "{at}: {what}").unwrap();
            }
        }
        for (twkb, why) in KNOWN {
            if !self.differences.iter().any(|(t, ..)| t == twkb) {
                writeln!(
                    failures,
                    "{twkb}: listed as known ({why}), but PostGIS agrees"
                )
                .unwrap();
            }
        }
        assert!(
            failures.is_empty(),
            "{} cases and {} random geometries checked against PostGIS; differences:\n{failures}",
            cases.len(),
            self.random
        );
        eprintln!(
            "{} cases and {} random geometries agree with PostGIS, but for the {} known differences",
            cases.len(),
            self.random,
            KNOWN.len()
        );
    }
}

/// The bytes of each geometry in a stream of them.
fn geometries(twkb: &[u8]) -> Vec<Vec<u8>> {
    let mut rest = twkb;
    let mut out = Vec::new();
    while !rest.is_empty() {
        let before = rest;
        Twkb::read(&mut rest).unwrap();
        out.push(before[..before.len() - rest.len()].to_vec());
    }
    out
}

/// PostGIS writes each case's WKT as the case's TWKB.
fn check_written(db: &mut Client, cases: &[Case], report: &mut Report) {
    let written: Vec<_> = cases
        .iter()
        .filter_map(|c| c.written.as_ref().map(|w| (c, w)))
        .collect();
    let rows = db
        .query(
            "SELECT pg_temp.twkb_of_wkt(w, xy, z, m, s, b)
             FROM unnest($1::text[], $2::int4[], $3::int4[], $4::int4[], $5::bool[], $6::bool[])
                 WITH ORDINALITY AS t(w, xy, z, m, s, b, i)
             ORDER BY i",
            &[
                &written.iter().map(|(_, w)| w.wkt).collect::<Vec<_>>(),
                &written
                    .iter()
                    .map(|(_, w)| i32::from(w.xy))
                    .collect::<Vec<_>>(),
                &written
                    .iter()
                    .map(|(_, w)| i32::from(w.z))
                    .collect::<Vec<_>>(),
                &written
                    .iter()
                    .map(|(_, w)| i32::from(w.m))
                    .collect::<Vec<_>>(),
                &written.iter().map(|(_, w)| w.size).collect::<Vec<_>>(),
                &written.iter().map(|(_, w)| w.bbox).collect::<Vec<_>>(),
            ],
        )
        .unwrap();
    for ((case, w), row) in written.iter().zip(rows) {
        let postgis: String = row.get(0);
        if postgis != case.twkb {
            let repeated = if w.removes_repeated {
                ""
            } else {
                " (repeated points kept)"
            };
            report.differ(
                case.twkb,
                case.at,
                format!(
                    "{} writes {}, PostGIS {postgis}{repeated}",
                    w.wkt, case.twkb
                ),
            );
        }
    }
}

/// PostGIS reads each TWKB as the geometry this crate reads, compared as WKB.
fn check_read(db: &mut Client, reads: &[(String, Vec<u8>)], report: &mut Report) {
    let rows = db
        .query(
            "SELECT pg_temp.wkb_of_twkb(t) FROM unnest($1::bytea[]) WITH ORDINALITY AS u(t, i) ORDER BY i",
            &[&reads.iter().map(|(_, t)| t.as_slice()).collect::<Vec<_>>()],
        )
        .unwrap();
    for ((at, twkb), row) in reads.iter().zip(rows) {
        let postgis: String = row.get(0);
        let mut ours = Vec::new();
        write_wkb(&mut ours, &Twkb::new(twkb).unwrap(), &WkbOptions::default()).unwrap();
        let ours_hex = to_hex(&ours);
        if postgis != ours_hex && !within_f64_division(twkb, &ours, &postgis) {
            let twkb = to_hex(twkb);
            let ours = ours_hex;
            report.differ(
                &twkb,
                at,
                format!("{twkb} reads as WKB {ours}, PostGIS {postgis}"),
            );
        }
    }
}

/// Random geometries written by this crate and by PostGIS; returns PostGIS's TWKB of each, to be read.
fn check_random(db: &mut Client, report: &mut Report) -> Vec<(String, Vec<u8>)> {
    let mut rng = Rng(SEED);
    let inputs: Vec<_> = (0..RANDOM).map(|_| Input::random(&mut rng)).collect();
    report.random = inputs.len();
    let rows = db
        .query(
            "SELECT pg_temp.twkb_of_wkb(w, xy, z, m, s, b)
             FROM unnest($1::bytea[], $2::int4[], $3::int4[], $4::int4[], $5::bool[], $6::bool[])
                 WITH ORDINALITY AS t(w, xy, z, m, s, b, i)
             ORDER BY i",
            &[
                &inputs.iter().map(|i| i.wkb.as_slice()).collect::<Vec<_>>(),
                &inputs
                    .iter()
                    .map(|i| i32::from(i.opts.precision.xy))
                    .collect::<Vec<_>>(),
                &inputs
                    .iter()
                    .map(|i| i32::from(i.opts.precision.z))
                    .collect::<Vec<_>>(),
                &inputs
                    .iter()
                    .map(|i| i32::from(i.opts.precision.m))
                    .collect::<Vec<_>>(),
                &inputs.iter().map(|i| i.opts.size).collect::<Vec<_>>(),
                &inputs.iter().map(|i| i.opts.bbox).collect::<Vec<_>>(),
            ],
        )
        .unwrap();
    let mut reads = Vec::new();
    for (n, (input, row)) in inputs.iter().zip(rows).enumerate() {
        let at = format!("random #{n}");
        let postgis: String = row.get(0);
        let mut ours = Vec::new();
        let ours = match write_geometry(&mut ours, &read_wkb(&input.wkb).unwrap(), &input.opts) {
            Ok(()) => to_hex(&ours),
            Err(e) => format!("error: {e}"),
        };
        if postgis != ours {
            let p = input.opts.precision;
            report.differ(
                &ours,
                &at,
                format!(
                    "WKB {} at ({}, {}, {}), size {}, bbox {}: twkb {ours}, PostGIS {postgis}",
                    to_hex(&input.wkb),
                    p.xy,
                    p.z,
                    p.m,
                    input.opts.size,
                    input.opts.bbox
                ),
            );
        }
        if !postgis.starts_with("error") {
            reads.push((at, hex(&postgis)));
        }
    }
    reads
}

/// `SplitMix64`: a fixed sequence from the seed, the same on every platform.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in `0..n`.
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// True `percent` times in 100.
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// A random geometry as ISO WKB, little-endian, and the options to write it with.
struct Input {
    wkb: Vec<u8>,
    opts: WriteOptions,
}

impl Input {
    #[expect(clippy::cast_possible_truncation, reason = "small numbers")]
    fn random(rng: &mut Rng) -> Self {
        // PostGIS writes x and y with -7 to 7 digits; the format's -8 it only reads
        let precision = Precision {
            xy: rng.below(15) as i8 - 7,
            z: rng.below(8) as u8,
            m: rng.below(8) as u8,
        };
        let mut g = Generator {
            // Projected coordinates and other large values, where the bytes must agree as well
            large: rng.chance(25),
            rng,
            xy: precision.xy,
            z: false,
            m: false,
            last: None,
            out: Vec::new(),
        };
        g.z = g.rng.chance(30);
        g.m = g.rng.chance(30);
        g.geometry(0);
        Self {
            wkb: g.out,
            opts: WriteOptions {
                precision,
                size: g.rng.chance(30),
                bbox: g.rng.chance(30),
                // PostGIS always leaves them out
                remove_repeated_points: true,
            },
        }
    }
}

/// Writes random geometries of one set of dimensions as WKB.
struct Generator<'a> {
    rng: &'a mut Rng,
    /// The precision of x and y: below 0, no ordinate is near a half of its unit, which PostGIS rounds otherwise.
    xy: i8,
    /// Whether ordinates are of 1e5 to 1e11 rather than within 180; below 0 digits, of at most 1e5 once scaled, as
    /// PostGIS's error near a half grows with the scaled value until, at 1e7, it rounds any value wrongly (`KNOWN`).
    large: bool,
    z: bool,
    m: bool,
    /// The last coordinate, which the next one repeats now and then.
    last: Option<Vec<f64>>,
    out: Vec<u8>,
}

impl Generator<'_> {
    fn dims(&self) -> usize {
        2 + usize::from(self.z) + usize::from(self.m)
    }

    fn header(&mut self, kind: u32) {
        let code = kind + if self.z { 1000 } else { 0 } + if self.m { 2000 } else { 0 };
        self.out.push(1);
        self.out.extend(code.to_le_bytes());
    }

    #[expect(clippy::cast_possible_truncation, reason = "counts below 10")]
    fn count(&mut self, n: u64) {
        self.out.extend((n as u32).to_le_bytes());
    }

    /// An ordinate: a longitude or latitude of 7 digits or fewer, now and then of more, or a whole number; or a large
    /// value of 2 digits or fewer.
    #[expect(clippy::cast_precision_loss, reason = "below 2^53")]
    fn ordinate(&mut self, d: usize) -> f64 {
        loop {
            let v = if self.large {
                let largest = if self.xy < 0 {
                    11.min(5 + u64::from(self.xy.unsigned_abs()))
                } else {
                    11
                };
                let magnitude = 10_u64.pow(5 + u32::try_from(self.rng.below(largest - 4)).unwrap());
                let digits = u32::try_from(self.rng.below(3)).unwrap();
                let span = 2 * magnitude * 10_u64.pow(digits);
                (self.rng.below(span) as f64) / 10_f64.powi(digits.cast_signed()) - magnitude as f64
            } else {
                let digits: i32 = [0, 1, 3, 7, 9][usize::try_from(self.rng.below(5)).unwrap()];
                let span = 360 * 10_u64.pow(digits.cast_unsigned());
                (self.rng.below(span) as f64) / 10_f64.powi(digits) - 180.0
            };
            if d >= 2 || self.xy >= 0 || !near_half(v / 10_f64.powi(-i32::from(self.xy))) {
                return v;
            }
        }
    }

    fn coord(&mut self) {
        let c = match &self.last {
            Some(last) if self.rng.chance(25) => last.clone(),
            _ => (0..self.dims()).map(|d| self.ordinate(d)).collect(),
        };
        for v in &c {
            self.out.extend(v.to_le_bytes());
        }
        self.last = Some(c);
    }

    fn points(&mut self, n: u64) {
        self.count(n);
        for _ in 0..n {
            self.coord();
        }
    }

    /// A ring: closed, of 4 points or more, or now and then empty.
    fn ring(&mut self) {
        let n = self.rng.below(5) + 3;
        self.count(n + 1);
        let start = self.out.len();
        for _ in 0..n {
            self.coord();
        }
        let first = self.out[start..start + 8 * self.dims()].to_vec();
        self.out.extend(first);
    }

    fn polygon(&mut self) {
        self.header(3);
        let rings = if self.rng.chance(10) {
            0
        } else {
            self.rng.below(3) + 1
        };
        self.count(rings);
        for _ in 0..rings {
            self.ring();
        }
    }

    fn point(&mut self) {
        self.header(1);
        if self.rng.chance(10) {
            // An empty point, as WKB has it
            for _ in 0..self.dims() {
                self.out.extend(f64::NAN.to_le_bytes());
            }
        } else {
            self.coord();
        }
    }

    fn line_string(&mut self) {
        self.header(2);
        let n = if self.rng.chance(10) {
            0
        } else {
            self.rng.below(5) + 2
        };
        self.points(n);
    }

    fn geometry(&mut self, depth: u32) {
        let kinds = if depth < 2 { 7 } else { 6 };
        match self.rng.below(kinds) + 1 {
            1 => self.point(),
            2 => self.line_string(),
            3 => self.polygon(),
            kind => {
                self.header(u32::try_from(kind).unwrap());
                let n = self.rng.below(4);
                self.count(n);
                for _ in 0..n {
                    match kind {
                        4 => self.point(),
                        5 => self.line_string(),
                        6 => self.polygon(),
                        _ => self.geometry(depth + 1),
                    }
                }
            }
        }
    }
}

/// Whether a scaled ordinate is close enough to a half for PostGIS's single-precision scaling to round it otherwise:
/// within 5e-8 of its value, with a margin.
fn near_half(scaled: f64) -> bool {
    (scaled.fract().abs() - 0.5).abs() <= 1e-7 * scaled.abs().max(1.0)
}
