//! This crate's own cases, for what the other sources' cases leave out: written as PostGIS writes them, which
//! `against_postgis` checks, except where its `KNOWN` says why not.

use crate::{Runner, case, cases};

cases!(repeated_points, empty_parts, negative_precision);

/// Repeated points are left out, as PostGIS does.
const REMOVES_REPEATED: bool = true;

/// A line string keeps 2 points and a ring 4.
pub(crate) fn repeated_points(r: &mut Runner) {
    case!(r, "LINESTRING(0 0,0 0,1 1)", 0, 0, 0, false, false, @"
    02000200000202
    LINESTRING(0 0,1 1)
    ");
    case!(r, "LINESTRING(0 0,0 0,0 0)", 0, 0, 0, false, false, @"
    02000200000000
    LINESTRING(0 0,0 0)
    ");
    case!(r, "MULTILINESTRING((0 0,0 0,1 1),(2 2,2 2))", 0, 0, 0, false, false, @"
    05000202000002020202020000
    MULTILINESTRING((0 0,1 1),(2 2,2 2))
    ");
    case!(r, "POLYGON((0 0,1 0,1 0,1 1,0 0))", 0, 0, 0, false, false, @"
    030001040000020000020101
    POLYGON((0 0,1 0,1 1,0 0))
    ");
    case!(r, "POLYGON((0 0,0 0,0 0,0 0,0 0))", 0, 0, 0, false, false, @"
    030001040000000000000000
    POLYGON((0 0,0 0,0 0,0 0))
    ");
    case!(r, "LINESTRING(0.1 0.1,0.2 0.2,1 1)", 0, 0, 0, false, true, @"
    0201000200020200000202
    LINESTRING(0 0,1 1)
    ");
}

/// A multi-geometry or a collection of only empty parts is empty.
pub(crate) fn empty_parts(r: &mut Runner) {
    case!(r, "GEOMETRYCOLLECTION Z(POLYGON Z EMPTY)", 0, 0, 0, false, false, @"
    071801
    GEOMETRYCOLLECTION Z EMPTY
    ");
    case!(r, "GEOMETRYCOLLECTION(POINT EMPTY,LINESTRING EMPTY)", 0, 0, 0, true, true, @"
    071200
    GEOMETRYCOLLECTION EMPTY
    ");
    case!(r, "GEOMETRYCOLLECTION(POINT(1 2),GEOMETRYCOLLECTION(POINT EMPTY))", 0, 0, 0, false, false, @"
    070002010002040710
    GEOMETRYCOLLECTION(POINT(1 2),GEOMETRYCOLLECTION EMPTY)
    ");
    case!(r, "MULTIPOLYGON(EMPTY,EMPTY)", 0, 0, 0, false, false, @"
    0610
    MULTIPOLYGON EMPTY
    ");
    case!(r, "MULTILINESTRING(EMPTY,(1 2,3 4))", 0, 0, 0, false, true, @"
    05010204040402000202040404
    MULTILINESTRING(EMPTY,(1 2,3 4))
    ");
}

/// Below 0 digits: x and y as PostGIS reads and writes them, but near a half, where its single-precision scaling rounds
/// otherwise (`KNOWN` in `against_postgis`). PostGIS writes -7 digits at the least, and reads -8.
pub(crate) fn negative_precision(r: &mut Runner) {
    case!(r, "POINT(150 -150)", -2, 0, 0, false, false, @"
    31000403
    POINT(200 -200)
    ");
    case!(r, "POINT(149 -151)", -2, 0, 0, false, false, @"
    31000203
    POINT(100 -200)
    ");
    case!(r, "POINT(3570264.95 0)", -1, 0, 0, false, false, @"
    1100c4ca2b00
    POINT(3570260 0)
    ");
    case!(r, "f1000200", @"POINT(100000000 0)");
    case!(r, "31000202", @"POINT(100 100)");
    case!(r, "3100f60100", @"POINT(12300 0)");
}
