//! Encodes a geometry, given as WKT or as hex WKB, as TWKB, and prints it as hex.
//!
//! ```text
//! cargo run --example encode -- 'LINESTRING(-71.0366718 42.3734493,-71.0367619 42.373312)' --precision 7
//! cargo run --example encode -- 0101000000000000000000f03f0000000000000040
//! cargo run --example encode -- 'POINT Z (1.25 2.5 3)' --precision 1,0 --size --bbox
//! ```
//!
//! Options: `--precision XY[,Z[,M]]` (decimal digits, 0 by default as in PostGIS), `--size`, `--bbox`, and
//! `--remove-repeated` to leave out repeated points as PostGIS's
//! [`ST_AsTWKB`](https://postgis.net/docs/ST_AsTWKB.html) does.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "an example prints its result"
)]

use std::env;
use std::error::Error;
use std::process::ExitCode;
use std::str::FromStr as _;

mod common;

use common::{from_hex, to_hex};
use twkb::{Precision, WriteOptions, write_geometry};
use wkb::reader::read_wkb;
use wkt::Wkt;

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
    let mut opts = WriteOptions::default();
    let mut input = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--precision" => {
                opts.precision = precision(&args.next().ok_or("--precision needs a value")?)?;
            }
            "--size" => opts.size = true,
            "--bbox" => opts.bbox = true,
            "--remove-repeated" => opts.remove_repeated_points = true,
            _ if input.is_none() => input = Some(arg),
            _ => return Err(format!("unexpected argument {arg}").into()),
        }
    }
    let input = input.ok_or("give a geometry as WKT or as hex WKB")?;

    let mut twkb = Vec::new();
    let from = if let Some(wkb) = from_hex(&input) {
        write_geometry(&mut twkb, &read_wkb(&wkb)?, &opts)?;
        format!("{} bytes of WKB", wkb.len())
    } else {
        write_geometry(&mut twkb, &Wkt::<f64>::from_str(&input)?, &opts)?;
        format!("{} bytes of WKT", input.len())
    };
    println!("{}", to_hex(&twkb));
    println!("{} bytes of TWKB, from {from}", twkb.len());
    Ok(())
}

/// `XY[,Z[,M]]`: the decimal digits of x and y, and of z and m.
fn precision(s: &str) -> Result<Precision, Box<dyn Error>> {
    let mut parts = s.split(',');
    let xy = parts.next().unwrap_or_default().parse()?;
    let z = parts.next().map(str::parse).transpose()?.unwrap_or(0);
    let m = parts.next().map(str::parse).transpose()?.unwrap_or(0);
    Ok(Precision { xy, z, m })
}
