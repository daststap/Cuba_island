//! Pure geometry for the island window: placement on a monitor and DPI conversion.
//!
//! "Physical" values are device pixels as reported by Win32 (`GetCursorPos`, monitor
//! rects); "logical" values are CSS pixels in the webview, i.e. physical / scale.

use crate::error::{AppError, AppResult};

/// A rectangle in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
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
}

/// Window size in physical px for a logical size on a monitor with `scale`.
pub fn physical_size((w, h): (f64, f64), scale: f64) -> (u32, u32) {
    // Small positive values: the float-to-int casts are exact after rounding.
    ((w * scale).round() as u32, (h * scale).round() as u32)
}

/// Top-left of a window of physical width `window_w`, centred on the monitor's top edge.
pub fn window_origin(monitor: PhysRect, window_w: u32) -> (i32, i32) {
    let window_w = i32::try_from(window_w).unwrap_or(i32::MAX);
    (monitor.x + (monitor.w - window_w) / 2, monitor.y)
}

/// Converts a physical screen point to logical px relative to the window.
pub fn to_logical((cx, cy): (i32, i32), (ox, oy): (i32, i32), scale: f64) -> (f64, f64) {
    (f64::from(cx - ox) / scale, f64::from(cy - oy) / scale)
}

/// Whether the cursor is pushed against the monitor's top edge within `half_width`
/// logical px of the monitor's horizontal centre.
pub fn in_edge_zone((cx, cy): (i32, i32), monitor: PhysRect, scale: f64, half_width: f64) -> bool {
    let within_monitor = cx >= monitor.x && cx < monitor.x + monitor.w;
    let centre = f64::from(monitor.x) + f64::from(monitor.w) / 2.0;
    within_monitor && cy <= monitor.y && (f64::from(cx) - centre).abs() <= half_width * scale
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

    #[test]
    fn centres_window_on_top_edge_at_100_percent() {
        let size = physical_size((800.0, 560.0), 1.0);
        assert_eq!(size, (800, 560));
        assert_eq!(window_origin(FHD, size.0), (560, 0));
    }

    #[test]
    fn centres_window_on_top_edge_at_150_percent() {
        let size = physical_size((800.0, 560.0), 1.5);
        assert_eq!(size, (1200, 840));
        assert_eq!(window_origin(FHD, size.0), (360, 0));
    }

    #[test]
    fn centres_window_on_secondary_monitor_left_of_primary() {
        let left = PhysRect {
            x: -2560,
            y: -200,
            w: 2560,
            h: 1440,
        };
        let size = physical_size((800.0, 560.0), 1.25);
        assert_eq!(size.0, 1000);
        assert_eq!(window_origin(left, size.0), (-2560 + 780, -200));
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
    fn edge_zone_scales_with_dpi() {
        // FHD centre is x = 960; 150 logical px at 150% = 225 physical px.
        assert!(in_edge_zone((960 + 225, 0), FHD, 1.5, 150.0));
        assert!(!in_edge_zone((960 + 226, 0), FHD, 1.5, 150.0));
        assert!(!in_edge_zone((960, 1), FHD, 1.5, 150.0));
    }

    #[test]
    fn edge_zone_ignores_other_monitors() {
        let right = PhysRect {
            x: 1920,
            y: 0,
            w: 1920,
            h: 1080,
        };
        assert!(!in_edge_zone((960, 0), right, 1.0, 150.0));
        assert!(in_edge_zone((2880, 0), right, 1.0, 150.0));
    }
}
