# This crate's own cases

- **Source:** written for this crate, for what the other sources leave out: repeated points at the least a line string
  or a ring keeps, multi-geometries and collections of only empty parts, and negative precisions.
- **License:** the crate's own, MIT or Apache 2.0.

The TWKB of each written case is what PostGIS's [`ST_AsTWKB`](https://postgis.net/docs/ST_AsTWKB.html) gives, which
`against_postgis.rs` checks, but for the differences its `KNOWN` lists.
