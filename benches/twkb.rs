//! Writing and reading a line string of 1,000 points as OpenStreetMap has them (7 digits, steps of a few metres), and
//! the same as WKB with the wkb crate for comparison: `cargo bench -p twkb`.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use geo_traits::{CoordTrait as _, GeometryTrait as _, LineStringTrait as _};
use twkb::{Precision, Twkb, WriteOptions, write_geometry, write_scaled_line_string};
use wkb::reader::read_wkb;
use wkb::writer::{WriteOptions as WkbOptions, write_geometry as write_wkb};

/// A wandering line of `n` points: integers at 7 digits, and as degrees.
#[expect(
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    reason = "small numbers"
)]
fn line(n: usize) -> (Vec<[i64; 2]>, geo_types::LineString) {
    let mut p = [-710_366_718_i64, 423_734_493];
    let mut stored = Vec::with_capacity(n);
    for i in 0..n as i64 {
        p[0] += (i * 7919) % 601 - 300;
        p[1] += (i * 104_729) % 401 - 200;
        stored.push(p);
    }
    let degrees = stored
        .iter()
        .map(|&[x, y]| (x as f64 / 1e7, y as f64 / 1e7))
        .collect();
    (stored, degrees)
}

fn bench(c: &mut Criterion) {
    let (stored, degrees) = line(1000);
    let opts = WriteOptions {
        precision: Precision::xy(7),
        ..WriteOptions::default()
    };
    let mut twkb = Vec::new();
    write_geometry(&mut twkb, &degrees, &opts).expect("the bench's line");
    let mut wkb = Vec::new();
    write_wkb(&mut wkb, &degrees, &WkbOptions::default()).expect("the bench's line");

    let mut g = c.benchmark_group("line of 1000 points");
    g.throughput(Throughput::Elements(1000));
    let mut out = Vec::with_capacity(wkb.len());
    g.bench_function("twkb: write scaled integers", |b| {
        b.iter(|| {
            out.clear();
            write_scaled_line_string(&mut out, 7, black_box(&stored).iter().copied())
                .expect("the bench's line");
        });
    });
    g.bench_function("twkb: write geo-types", |b| {
        b.iter(|| {
            out.clear();
            write_geometry(&mut out, black_box(&degrees), &opts).expect("the bench's line");
        });
    });
    g.bench_function("twkb: read, sum the stored integers", |b| {
        b.iter(|| {
            Twkb::new(black_box(&twkb))
                .expect("the bench's line")
                .stored_coords()
                .iter()
                .sum::<i64>()
        });
    });
    g.bench_function("twkb: read, sum the f64 ordinates", |b| {
        b.iter(|| {
            let Twkb::LineString(l) = Twkb::new(black_box(&twkb)).expect("the bench's line") else {
                unreachable!()
            };
            l.coords().map(|c| c.x() + c.y()).sum::<f64>()
        });
    });
    g.bench_function("wkb: write geo-types", |b| {
        b.iter(|| {
            out.clear();
            write_wkb(&mut out, black_box(&degrees), &WkbOptions::default())
                .expect("the bench's line");
        });
    });
    g.bench_function("wkb: read, sum the f64 ordinates", |b| {
        b.iter(|| {
            let g = read_wkb(black_box(&wkb)).expect("the bench's line");
            let geo_traits::GeometryType::LineString(l) = g.as_type() else {
                unreachable!()
            };
            l.coords().map(|c| c.x() + c.y()).sum::<f64>()
        });
    });
    g.finish();
    #[expect(clippy::print_stdout, reason = "the sizes, beside the times")]
    {
        println!("sizes: twkb {} bytes, wkb {} bytes", twkb.len(), wkb.len());
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
