//! The wkb crate's sample geometries, as WKT: see `README.md` beside this file for where they come from and their
//! license.

/// Each sample, its rings closed as geo-types closes them.
pub const SAMPLES: [&str; 8] = [
    // point_2d
    "POINT(0 1)",
    // linestring_2d
    "LINESTRING(0 1,1 2)",
    // polygon_2d
    "POLYGON((-111 45,-111 41,-104 41,-104 45,-111 45))",
    // polygon_2d_with_interior
    "POLYGON((-111 45,-111 41,-104 41,-104 45,-111 45),(-110 44,-110 42,-105 42,-105 44,-110 44))",
    // multi_point_2d
    "MULTIPOINT((0 1),(1 2))",
    // multi_line_string_2d
    "MULTILINESTRING((-111 45,-111 41,-104 41,-104 45),(-110 44,-110 42,-105 42,-105 44))",
    // multi_polygon_2d
    "MULTIPOLYGON(((-111 45,-111 41,-104 41,-104 45,-111 45)),((-111 45,-111 41,-104 41,-104 45,-111 45),(-110 44,-110 42,-105 42,-105 44,-110 44)))",
    // geometry_collection_2d
    "GEOMETRYCOLLECTION(POINT(0 1),LINESTRING(0 1,1 2),POLYGON((-111 45,-111 41,-104 41,-104 45,-111 45)),POLYGON((-111 45,-111 41,-104 41,-104 45,-111 45),(-110 44,-110 42,-105 42,-105 44,-110 44)),MULTIPOINT((0 1),(1 2)),MULTILINESTRING((-111 45,-111 41,-104 41,-104 45),(-110 44,-110 42,-105 42,-105 44)),MULTIPOLYGON(((-111 45,-111 41,-104 41,-104 45,-111 45)),((-111 45,-111 41,-104 41,-104 45,-111 45),(-110 44,-110 42,-105 42,-105 44,-110 44))))",
];
