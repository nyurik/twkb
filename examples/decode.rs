//! Decodes a hex TWKB geometry, and prints it as WKT and as hex WKB, with what its header holds.
//!
//! ```text
//! cargo run --example decode -- e20002fbd7baa505bab38d9403890eb915
//! cargo run --example decode -- 04070b0004020402000200020404
//! ```
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "an example prints its result"
)]

use std::env;
use std::error::Error;
use std::process::ExitCode;

mod common;

use common::{from_hex, to_hex};
use twkb::Twkb;
use wkb::writer::{WriteOptions, write_geometry};
use wkt::to_wkt::write_geometry as write_wkt;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let input = env::args().nth(1).ok_or("give a TWKB geometry as hex")?;
    let twkb = from_hex(&input).ok_or("not hex")?;
    let geometry = Twkb::new(&twkb)?;

    let mut wkt = String::new();
    write_wkt(&mut wkt, &geometry)?;
    let mut wkb = Vec::new();
    write_geometry(&mut wkb, &geometry, &WriteOptions::default())?;
    println!("WKT: {wkt}");
    println!("WKB: {}", to_hex(&wkb));

    let header = geometry.header();
    let p = header.precision();
    println!(
        "{:?}, {:?}, {} digits for x and y, {} for z, {} for m; {} bytes of TWKB, {} of WKB",
        header.geometry_type(),
        header.dimension(),
        p.xy,
        p.z,
        p.m,
        twkb.len(),
        wkb.len()
    );
    if let Some(size) = header.size() {
        println!("size: {size} bytes");
    }
    if let Some(bbox) = header.bbox() {
        println!("bounding box: {:?}", bbox.collect::<Vec<_>>());
    }
    if let Some(ids) = header.ids() {
        println!("IDs: {ids:?}");
    }
    Ok(())
}
