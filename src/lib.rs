#![doc = include_str!("../README.md")]

mod common;
mod error;
mod reader;
mod writer;

pub use crate::common::{Dimension, GeometryType, Precision};
pub use crate::error::TwkbError;
pub use crate::reader::{
    Coord, GeometryCollection, Header, LineString, LineStringRef, MultiLineString, MultiPoint,
    MultiPolygon, Point, PointRef, Polygon, PolygonRef, ReadOptions, Twkb,
};
pub use crate::writer::{
    WriteOptions, write_geometry, write_scaled_line_string, write_scaled_point,
};
