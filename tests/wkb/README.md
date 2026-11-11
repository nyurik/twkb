# From wkb

- **Source:** <https://github.com/georust/wkb>, commit `f15fa9ccabcfbe064ef211f2f0ceec1396e70ab6` (version 0.9.3).
- **License:** MIT or Apache 2.0 (`LICENSE-MIT`, `LICENSE-APACHE`, copied from the crate).

| File | What it holds |
|---|---|
| `samples.rs` | The crate's sample geometries from [`data.rs`](https://github.com/georust/wkb/blob/f15fa9ccabcfbe064ef211f2f0ceec1396e70ab6/src/test/data.rs), as WKT, the rings closed as geo-types closes them: `tests/wkb_samples.rs` writes them as TWKB and reads them back, and passes them through the wkb crate |
