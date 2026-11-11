//! The cases from this directory's source: see `README.md` beside this file for where they come from and their
//! license.

use crate::{Runner, case, cases};

cases!(cases);

/// Whether the source's TWKB comes from PostGIS, which leaves out repeated points.
const REMOVES_REPEATED: bool = false;

pub(crate) fn cases(r: &mut Runner) {
    case!(r, "01000204",
        @"POINT(1 2)");
    case!(r, "02000202020808",
        @"LINESTRING(1 1,5 5)");
    case!(r, "03031b000400040205000004000004030000030500000002020000010100",
        @"POLYGON((0 0,2 0,2 2,0 2,0 0),(0 0,0 1,1 1,1 0,0 0))");
    case!(r, "04070b0004020402000200020404",
        @"MULTIPOINT((0 1),(2 3)) ids [0, 1]");
    case!(r, "070402000201000002020002080a0404",
        @"GEOMETRYCOLLECTION(POINT(0 1),LINESTRING(4 5,6 7)) ids [0, 1]");
    case!(r, "MULTIPOINT((1 2),(3 4))", 0, 0, 0, true, true, @"
    040309020404040202040404
    MULTIPOINT((1 2),(3 4))
    ");
    case!(r, "MULTILINESTRING((1 2,3 4),(5 6,7 8))", 0, 0, 0, true, true, @"
    05030f020c040c0202020404040204040404
    MULTILINESTRING((1 2,3 4),(5 6,7 8))
    ");
    case!(r, "MULTIPOLYGON(((0 0,1 0,1 1,0 1,0 0)),((10 10,11 10,11 11,10 11,10 10)))", 0, 0, 0, true, true, @"
    06031d0016001602010500000200000201000001010514140200000201000001
    MULTIPOLYGON(((0 0,1 0,1 1,0 1,0 0)),((10 10,11 10,11 11,10 11,10 10)))
    ");
}
