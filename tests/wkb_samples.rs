//! The wkb crate's sample geometries (`tests/wkb/samples.rs`, with its license) written as TWKB at several
//! precisions and read back, each ordinate within half a unit of the last digit kept; the same through the wkb crate, as WKB both crates read
//! and write with geo-traits; and into geo-types with geo-traits' conversion.
#![allow(clippy::unwrap_used, reason = "a test fails on any error")]

mod common;
#[path = "wkb/samples.rs"]
mod samples;

use common::{same, shape};
use geo_traits::to_geo::ToGeoGeometry as _;
use geo_types::Geometry;
use std::str::FromStr as _;

use twkb::{Precision, Twkb, WriteOptions, write_geometry};
use wkb::reader::read_wkb;
use wkb::writer::{WriteOptions as WkbOptions, write_geometry as write_wkb};
use wkt::Wkt;

fn samples() -> Vec<Geometry> {
    samples::SAMPLES
        .iter()
        .map(|wkt| Wkt::<f64>::from_str(wkt).unwrap().to_geometry())
        .collect()
}

/// A sample moved off whole numbers, so that rounding matters.
fn shifted(g: &mut Geometry) {
    let shift = |c: &mut geo_types::Coord| {
        c.x += 0.123_456_789_123;
        c.y -= 0.987_654_321_987;
    };
    let line = |l: &mut geo_types::LineString| l.coords_mut().for_each(shift);
    let polygon = |p: &mut geo_types::Polygon| {
        p.exterior_mut(line);
        p.interiors_mut(|rings| rings.iter_mut().for_each(line));
    };
    match g {
        Geometry::Point(p) => shift(&mut p.0),
        Geometry::LineString(l) => line(l),
        Geometry::Polygon(p) => polygon(p),
        Geometry::MultiPoint(m) => m.0.iter_mut().for_each(|p| shift(&mut p.0)),
        Geometry::MultiLineString(m) => m.0.iter_mut().for_each(line),
        Geometry::MultiPolygon(m) => m.0.iter_mut().for_each(polygon),
        Geometry::GeometryCollection(c) => c.0.iter_mut().for_each(shifted),
        Geometry::Line(_) | Geometry::Rect(_) | Geometry::Triangle(_) => {
            unreachable!("not among the samples")
        }
    }
}

#[test]
fn round_trips_within_precision() {
    for g in samples()
        .into_iter()
        .chain(samples().into_iter().map(|mut g| {
            shifted(&mut g);
            g
        }))
    {
        for xy in [-3, -1, 0, 1, 3, 5, 7] {
            for (size, bbox) in [(false, false), (true, true)] {
                let opts = WriteOptions {
                    precision: Precision::xy(xy),
                    size,
                    bbox,
                    ..WriteOptions::default()
                };
                let mut buf = Vec::new();
                write_geometry(&mut buf, &g, &opts).unwrap();
                let read = Twkb::new(&buf).unwrap();
                let half = 0.5 * 10_f64.powi(-i32::from(xy)) * (1.0 + 1e-12);
                assert!(
                    same(&shape(&read), &shape(&g), &|_| half),
                    "{g:?} at {xy} decimals: {read:?}"
                );
                assert_eq!(read.header().size().is_some(), size);
            }
        }
    }
}

#[test]
fn reads_what_the_wkb_crate_reads() {
    for g in samples() {
        let opts = WriteOptions {
            precision: Precision::xy(7),
            ..WriteOptions::default()
        };
        let mut direct = Vec::new();
        write_geometry(&mut direct, &g, &opts).unwrap();
        let mut wkb = Vec::new();
        write_wkb(&mut wkb, &g, &WkbOptions::default()).unwrap();
        let mut through_wkb = Vec::new();
        write_geometry(&mut through_wkb, &read_wkb(&wkb).unwrap(), &opts).unwrap();
        assert_eq!(direct, through_wkb, "{g:?}");
    }
}

#[test]
fn converts_into_geo_types() {
    for g in samples() {
        let mut buf = Vec::new();
        write_geometry(&mut buf, &g, &WriteOptions::default()).unwrap();
        let read: Twkb = Twkb::new(&buf).unwrap();
        assert_eq!(read.to_geometry(), g);
    }
}
