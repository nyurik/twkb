# From PostGIS

- **Source:** <https://github.com/postgis/postgis>, commit `c87677a620f814978f9eb3dced943c2b1241d149` (2026-11-10).
- **License:** GNU General Public License 2.0 or later (`COPYING`, copied from the repository). Only test cases are taken, as data, not PostGIS's code; they stay in the repository, out of the published crate.

| File | What it holds |
|---|---|
| `mod.rs` | Each statement of [`twkb.sql`](https://github.com/postgis/postgis/blob/c87677a620f814978f9eb3dced943c2b1241d149/regress/core/twkb.sql) with its line of [`twkb_expected`](https://github.com/postgis/postgis/blob/c87677a620f814978f9eb3dced943c2b1241d149/regress/core/twkb_expected): points, line strings, polygons with holes, multi-geometries, collections, a bounding box in a collection, a large multi-polygon, and the removal of repeated points from line strings and rings but not multi-points |
