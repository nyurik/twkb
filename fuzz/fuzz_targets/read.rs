//! Any bytes: reading never panics, and what reads is written and read back the same.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| twkb_fuzz::read(data));
