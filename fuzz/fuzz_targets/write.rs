//! Any geometry and options: writing never panics, and what is written reads back within its precision.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: twkb_fuzz::WriteInput| input.check());
