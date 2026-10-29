//! Chamfered shapes: Bolt surfaces cut at 45 degrees instead of rounded.
//!
//! GPUI has no `clip-path`, so shapes are painted as polygons. [`geometry`] holds the pure
//! outline math.

pub mod geometry;

pub use geometry::{
    CutCorners, clamp_cut, cut_outline, edge_polygons, outside_polygons, snap_bounds_to_device,
    snap_length_to_device, snap_offset_to_device,
};
