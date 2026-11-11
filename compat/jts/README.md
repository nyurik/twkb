# From JTS Topology Suite

- **Source:** <https://github.com/locationtech/jts>, commit `3460207216f3091cd7f72d48d6ddd1b561b2b8fd` (2026-10-29).
- **License:** Eclipse Public License 2.0 or Eclipse Distribution License 1.0, a BSD-3-Clause license (`LICENSE_EPLv2.txt`, `LICENSE_EDLv1.txt`, copied from the repository).

| File | What it holds |
|---|---|
| `mod.rs`, `points` to `geometrycollections` | The rows of each data file of [`modules/io/common/src/test/resources/testdata/twkb/`](https://github.com/locationtech/jts/tree/3460207216f3091cd7f72d48d6ddd1b561b2b8fd/modules/io/common/src/test/resources/testdata/twkb/), which [`generate_test_data.sql`](https://github.com/locationtech/jts/blob/3460207216f3091cd7f72d48d6ddd1b561b2b8fd/modules/io/common/src/test/resources/testdata/twkb/generate_test_data.sql) made with PostGIS: each input WKT written with every combination of precision (-7, 0, 7 for x and y; 0, 7 for z and m), size and bounding box, the TWKB it gave, and the WKT PostGIS read back from it. Z, M and ZM, empty parts, and points that rounding collapses |
| `mod.rs`, `writer_and_reader_tests` | The cases of [`TWKBWriterTest.java`](https://github.com/locationtech/jts/blob/3460207216f3091cd7f72d48d6ddd1b561b2b8fd/modules/io/common/src/test/java/org/locationtech/jts/io/twkb/TWKBWriterTest.java) (empty geometries with sizes and boxes; every x and y precision from -7 to 7, and every z and m precision of one point) and [`TWKBReaderTest.java`](https://github.com/locationtech/jts/blob/3460207216f3091cd7f72d48d6ddd1b561b2b8fd/modules/io/common/src/test/java/org/locationtech/jts/io/twkb/TWKBReaderTest.java) (z and m precisions) |
