//! Pure geometry for the island window: placement on a monitor and DPI conversion.
//!
//! "Physical" values are device pixels as reported by Win32 (`GetCursorPos`, monitor
//! rects); "logical" values are CSS pixels in the webview, i.e. physical / scale.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// A rectangle in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// The screen edge the island is attached to. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    /// Left and right: the pill stands upright and the window is taller than wide.
    pub fn is_side(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// Fixed logical window size for an edge; only the island inside it changes shape.
/// Wide enough on every edge for the expanded panel (640 × 360) and peeks.
pub fn window_size(edge: Edge) -> (f64, f64) {
    if edge.is_side() {
        (680.0, 800.0)
    } else {
        (800.0, 560.0)
    }
}

/// The visible island inside the window, in logical px relative to the window's top-left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IslandRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl IslandRect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> AppResult<Self> {
        let finite = [x, y, w, h].iter().all(|v| v.is_finite());
        if !finite || w < 0.0 || h < 0.0 {
            return Err(AppError::InvalidArgument(format!(
                "island rect must be finite with non-negative size, got {x},{y} {w}x{h}"
            )));
        }
        Ok(Self { x, y, w, h })
    }

    /// Whether a logical point lies inside the rect grown by `margin` on every side.
    /// An empty rect (not reported yet) contains nothing.
    pub fn contains(&self, (px, py): (f64, f64), margin: f64) -> bool {
        if self.w <= 0.0 || self.h <= 0.0 {
            return false;
        }
        px >= self.x - margin
            && px <= self.x + self.w + margin
            && py >= self.y - margin
            && py <= self.y + self.h + margin
    }
}

/// Where the island window currently sits; everything the cursor tracker needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub window_origin: (i32, i32),
    pub scale: f64,
    pub monitor: PhysRect,
    pub edge: Edge,
    /// Physical coordinate of the island's centre along the edge (x for top/bottom).
    pub centre: i32,
    /// The island touches the monitor's edge (no taskbar in between), so pushing
    /// the cursor against that edge may open it.
    pub flush: bool,
}

/// Window size in physical px for a logical size on a monitor with `scale`.
pub fn physical_size((w, h): (f64, f64), scale: f64) -> (u32, u32) {
    // Small positive values: the float-to-int casts are exact after rounding.
    ((w * scale).round() as u32, (h * scale).round() as u32)
}

/// The window position for an edge and an `offset` (0–1) along it, inside `area`
/// (the monitor's work area), plus the island centre along the edge. The window
/// stays inside the area; near a corner the island simply sits off-centre in it.
pub fn place(area: PhysRect, edge: Edge, offset: f64, (w, h): (u32, u32)) -> ((i32, i32), i32) {
    let (w, h) = (
        i32::try_from(w).unwrap_or(i32::MAX),
        i32::try_from(h).unwrap_or(i32::MAX),
    );
    let offset = if offset.is_finite() {
        offset.clamp(0.0, 1.0)
    } else {
        0.5
    };
    let along = |start: i32, len: i32, size: i32| {
        let centre = start + (f64::from(len) * offset).round() as i32;
        let origin = (centre - size / 2).clamp(start, (start + len - size).max(start));
        (origin, centre)
    };
    match edge {
        Edge::Top => {
            let (x, c) = along(area.x, area.w, w);
            ((x, area.y), c)
        }
        Edge::Bottom => {
            let (x, c) = along(area.x, area.w, w);
            ((x, area.y + area.h - h), c)
        }
        Edge::Left => {
            let (y, c) = along(area.y, area.h, h);
            ((area.x, y), c)
        }
        Edge::Right => {
            let (y, c) = along(area.y, area.h, h);
            ((area.x + area.w - w, y), c)
        }
    }
}

/// Whether the work area's `edge` coincides with the monitor's (no taskbar there).
pub fn is_flush(monitor: PhysRect, area: PhysRect, edge: Edge) -> bool {
    match edge {
        Edge::Top => area.y == monitor.y,
        Edge::Bottom => area.y + area.h == monitor.y + monitor.h,
        Edge::Left => area.x == monitor.x,
        Edge::Right => area.x + area.w == monitor.x + monitor.w,
    }
}

/// Where a dropped island goes: the edge of `area` nearest to the cursor and the
/// cursor's position (0–1) along that edge.
pub fn nearest_edge(area: PhysRect, (cx, cy): (i32, i32)) -> (Edge, f64) {
    let distances = [
        (Edge::Top, cy - area.y),
        (Edge::Bottom, area.y + area.h - cy),
        (Edge::Left, cx - area.x),
        (Edge::Right, area.x + area.w - cx),
    ];
    let edge = distances
        .iter()
        .min_by_key(|(_, d)| *d)
        .map_or(Edge::Top, |(edge, _)| *edge);
    let fraction = |pos: i32, start: i32, len: i32| {
        if len <= 0 {
            0.5
        } else {
            (f64::from(pos - start) / f64::from(len)).clamp(0.0, 1.0)
        }
    };
    let offset = if edge.is_side() {
        fraction(cy, area.y, area.h)
    } else {
        fraction(cx, area.x, area.w)
    };
    (edge, offset)
}

/// Converts a physical screen point to logical px relative to the window.
pub fn to_logical((cx, cy): (i32, i32), (ox, oy): (i32, i32), scale: f64) -> (f64, f64) {
    (f64::from(cx - ox) / scale, f64::from(cy - oy) / scale)
}

/// Whether the cursor is pushed against the island's monitor edge within
/// `half_width` logical px of the island's centre along that edge.
pub fn in_edge_zone((cx, cy): (i32, i32), placement: &Placement, half_width: f64) -> bool {
    if !placement.flush {
        return false;
    }
    let m = placement.monitor;
    let reach = half_width * placement.scale;
    let near = |pos: i32| (f64::from(pos) - f64::from(placement.centre)).abs() <= reach;
    let in_x = cx >= m.x && cx < m.x + m.w;
    let in_y = cy >= m.y && cy < m.y + m.h;
    match placement.edge {
        Edge::Top => in_x && cy <= m.y && near(cx),
        Edge::Bottom => in_x && cy >= m.y + m.h - 1 && near(cx),
        Edge::Left => in_y && cx <= m.x && near(cy),
        Edge::Right => in_y && cx >= m.x + m.w - 1 && near(cy),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FHD: PhysRect = PhysRect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
    };
    /// FHD minus a 48 px taskbar at the bottom.
    const WORK: PhysRect = PhysRect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1032,
    };

    fn placement(edge: Edge, centre: i32, scale: f64) -> Placement {
        Placement {
            window_origin: (0, 0),
            scale,
            monitor: FHD,
            edge,
            centre,
            flush: true,
        }
    }

    #[test]
    fn centres_on_the_top_edge_like_before() {
        let size = physical_size(window_size(Edge::Top), 1.0);
        assert_eq!(size, (800, 560));
        assert_eq!(place(FHD, Edge::Top, 0.5, size), ((560, 0), 960));
        let size = physical_size(window_size(Edge::Top), 1.5);
        assert_eq!(place(FHD, Edge::Top, 0.5, size), ((360, 0), 960));
    }

    #[test]
    fn sits_above_the_taskbar_at_the_bottom() {
        let size = physical_size(window_size(Edge::Bottom), 1.0);
        assert_eq!(
            place(WORK, Edge::Bottom, 0.5, size),
            ((560, 1032 - 560), 960)
        );
    }

    #[test]
    fn stands_upright_on_the_sides() {
        let size = physical_size(window_size(Edge::Left), 1.0);
        assert_eq!(size, (680, 800));
        assert_eq!(place(FHD, Edge::Left, 0.5, size), ((0, 140), 540));
        assert_eq!(place(FHD, Edge::Right, 0.5, size), ((1920 - 680, 140), 540));
    }

    #[test]
    fn keeps_the_window_on_screen_near_corners() {
        let size = physical_size(window_size(Edge::Top), 1.0);
        // Centre at x = 96, but the window cannot start left of the monitor.
        assert_eq!(place(FHD, Edge::Top, 0.05, size), ((0, 0), 96));
        assert_eq!(place(FHD, Edge::Top, 1.0, size), ((1120, 0), 1920));
        // Nonsense offsets fall back to the middle.
        assert_eq!(place(FHD, Edge::Top, f64::NAN, size).1, 960);
    }

    #[test]
    fn places_on_a_secondary_monitor_left_of_primary() {
        let left = PhysRect {
            x: -2560,
            y: -200,
            w: 2560,
            h: 1440,
        };
        let size = physical_size(window_size(Edge::Top), 1.25);
        assert_eq!(size.0, 1000);
        assert_eq!(place(left, Edge::Top, 0.5, size).0, (-2560 + 780, -200));
    }

    #[test]
    fn drops_onto_the_nearest_edge() {
        assert_eq!(nearest_edge(FHD, (960, 20)), (Edge::Top, 0.5));
        assert_eq!(nearest_edge(FHD, (480, 1070)), (Edge::Bottom, 0.25));
        let (edge, offset) = nearest_edge(FHD, (5, 270));
        assert_eq!((edge, offset), (Edge::Left, 0.25));
        assert_eq!(nearest_edge(FHD, (1915, 540)).0, Edge::Right);
    }

    #[test]
    fn knows_when_the_taskbar_is_in_between() {
        assert!(is_flush(FHD, WORK, Edge::Top));
        assert!(!is_flush(FHD, WORK, Edge::Bottom));
        assert!(is_flush(FHD, WORK, Edge::Left));
    }

    #[test]
    fn converts_cursor_to_logical_with_dpi() {
        assert_eq!(to_logical((660, 18), (560, 0), 1.0), (100.0, 18.0));
        assert_eq!(to_logical((510, 27), (360, 0), 1.5), (100.0, 18.0));
    }

    #[test]
    fn contains_respects_margin() {
        let rect = IslandRect::new(300.0, 0.0, 200.0, 36.0).unwrap();
        assert!(rect.contains((300.0, 10.0), 0.0));
        assert!(rect.contains((297.0, 39.0), 4.0));
        assert!(!rect.contains((295.0, 10.0), 4.0));
        assert!(!IslandRect::default().contains((0.0, 0.0), 4.0));
    }

    #[test]
    fn rejects_invalid_rects() {
        assert!(IslandRect::new(0.0, 0.0, -1.0, 10.0).is_err());
        assert!(IslandRect::new(f64::NAN, 0.0, 1.0, 10.0).is_err());
    }

    #[test]
    fn edge_zone_follows_the_island_and_scales_with_dpi() {
        // Top, centre 960; 150 logical px at 150% = 225 physical px.
        let top = placement(Edge::Top, 960, 1.5);
        assert!(in_edge_zone((960 + 225, 0), &top, 150.0));
        assert!(!in_edge_zone((960 + 226, 0), &top, 150.0));
        assert!(!in_edge_zone((960, 1), &top, 150.0));
        // Left edge, island at y = 300.
        let left = placement(Edge::Left, 300, 1.0);
        assert!(in_edge_zone((0, 300), &left, 150.0));
        assert!(!in_edge_zone((0, 600), &left, 150.0));
        // Bottom edge behind a taskbar: no push.
        let bottom = Placement {
            flush: false,
            ..placement(Edge::Bottom, 960, 1.0)
        };
        assert!(!in_edge_zone((960, 1079), &bottom, 150.0));
    }

    #[test]
    fn edge_zone_ignores_other_monitors() {
        let right_monitor = Placement {
            monitor: PhysRect {
                x: 1920,
                y: 0,
                w: 1920,
                h: 1080,
            },
            ..placement(Edge::Top, 2880, 1.0)
        };
        assert!(!in_edge_zone((960, 0), &right_monitor, 150.0));
        assert!(in_edge_zone((2880, 0), &right_monitor, 150.0));
    }
}
