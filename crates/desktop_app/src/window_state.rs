//! Remembers the size, position, and windowed/maximized/fullscreen state of
//! the last terminal window so the next launch opens where the user left it.
//!
//! The state lives in its own file next to the config so the user's
//! `window_width`/`window_height` defaults are never rewritten.

use gpui_kit::{
    App, Bounds, DisplayId, Global, Pixels, Size, Task, Window, WindowBounds, point, px, size,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const WINDOW_STATE_FILE: &str = "window-state.json";
/// Moving or resizing fires bounds changes every frame; only the final
/// geometry is worth writing.
const SAVE_DELAY: Duration = Duration::from_millis(400);
/// A restored window keeps at least this much of its top edge on screen so
/// it can still be grabbed and moved.
const MIN_VISIBLE_EDGE: f32 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WindowMode {
    Windowed,
    Maximized,
    Fullscreen,
}

/// Window geometry as saved on disk. `x`/`y`/`width`/`height` are the
/// windowed bounds, also used as the restore size for maximized and
/// fullscreen windows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SavedWindowState {
    mode: WindowMode,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display_uuid: Option<String>,
}

pub(crate) struct RestoredWindowGeometry {
    pub(crate) bounds: WindowBounds,
    pub(crate) display_id: Option<DisplayId>,
}

#[derive(Default)]
struct WindowStateWriter {
    unsaved: Option<SavedWindowState>,
    save_task: Option<Task<()>>,
}

impl Global for WindowStateWriter {}

impl SavedWindowState {
    fn capture(window: &Window, cx: &App) -> Option<Self> {
        let (mode, bounds) = match window.window_bounds() {
            WindowBounds::Windowed(bounds) => (WindowMode::Windowed, bounds),
            WindowBounds::Maximized(bounds) => (WindowMode::Maximized, bounds),
            WindowBounds::Fullscreen(bounds) => (WindowMode::Fullscreen, bounds),
        };
        let state = Self {
            mode,
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
            display_uuid: window
                .display(cx)
                .and_then(|display| display.uuid().ok())
                .map(|uuid| uuid.to_string()),
        };
        state.is_valid().then_some(state)
    }

    fn is_valid(&self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|value| value.is_finite())
            && self.width > 0.0
            && self.height > 0.0
    }

    fn resolve(
        &self,
        cx: &App,
        min_size: Size<Pixels>,
        restore_position: bool,
    ) -> RestoredWindowGeometry {
        let window_size = size(
            px(self.width).max(min_size.width),
            px(self.height).max(min_size.height),
        );
        if !restore_position {
            return RestoredWindowGeometry {
                bounds: WindowBounds::Windowed(Bounds::centered(None, window_size, cx)),
                display_id: None,
            };
        }

        let saved_display = self.display_uuid.as_deref().and_then(|saved_uuid| {
            cx.displays().into_iter().find(|display| {
                display
                    .uuid()
                    .is_ok_and(|uuid| uuid.to_string() == saved_uuid)
            })
        });
        // The saved position only means something on the display it was
        // saved on; anywhere else, center the saved size instead.
        let position_is_known = saved_display.is_some() || self.display_uuid.is_none();
        let Some(display) = saved_display.or_else(|| cx.primary_display()) else {
            return RestoredWindowGeometry {
                bounds: WindowBounds::Windowed(Bounds::centered(None, window_size, cx)),
                display_id: None,
            };
        };
        let bounds = if position_is_known {
            fit_bounds_to_display(
                Bounds::new(point(px(self.x), px(self.y)), window_size),
                display.bounds(),
            )
        } else {
            Bounds::centered(Some(display.id()), window_size, cx)
        };

        RestoredWindowGeometry {
            bounds: match self.mode {
                WindowMode::Windowed => WindowBounds::Windowed(bounds),
                WindowMode::Maximized => WindowBounds::Maximized(bounds),
                WindowMode::Fullscreen => WindowBounds::Fullscreen(bounds),
            },
            display_id: Some(display.id()),
        }
    }
}

/// Shrinks `window` to fit on `display` and recenters it when too little of
/// its top edge would remain reachable, e.g. after a display layout change.
fn fit_bounds_to_display(window: Bounds<Pixels>, display: Bounds<Pixels>) -> Bounds<Pixels> {
    let window_size = size(
        window.size.width.min(display.size.width),
        window.size.height.min(display.size.height),
    );
    let left = f32::from(window.origin.x);
    let top = f32::from(window.origin.y);
    let width = f32::from(window_size.width);
    let display_left = f32::from(display.origin.x);
    let display_top = f32::from(display.origin.y);
    let display_right = display_left + f32::from(display.size.width);
    let display_bottom = display_top + f32::from(display.size.height);

    let visible_width = (left + width).min(display_right) - left.max(display_left);
    let top_is_reachable = top >= display_top && top <= display_bottom - MIN_VISIBLE_EDGE;
    if visible_width >= MIN_VISIBLE_EDGE.min(width) && top_is_reachable {
        return Bounds::new(window.origin, window_size);
    }

    Bounds::new(
        point(
            display.origin.x + (display.size.width - window_size.width) / 2.0,
            display.origin.y + (display.size.height - window_size.height) / 2.0,
        ),
        window_size,
    )
}

fn window_state_path() -> Option<PathBuf> {
    let config_path = termy_core::config_core::config_path()?;
    Some(config_path.parent()?.join(WINDOW_STATE_FILE))
}

fn load() -> Option<SavedWindowState> {
    let path = window_state_path()?;
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            log::warn!("Failed to read window state '{}': {error}", path.display());
            return None;
        }
    };
    match serde_json::from_str::<SavedWindowState>(&contents) {
        Ok(state) if state.is_valid() => Some(state),
        Ok(_) => None,
        Err(error) => {
            log::warn!(
                "Ignoring invalid window state '{}': {error}",
                path.display()
            );
            None
        }
    }
}

fn write(state: &SavedWindowState) {
    let Some(path) = window_state_path() else {
        return;
    };
    let contents = match serde_json::to_string_pretty(state) {
        Ok(contents) => contents,
        Err(error) => {
            log::warn!("Failed to encode window state: {error}");
            return;
        }
    };
    if let Err(error) = crate::config::write_atomic(&path, &contents) {
        log::warn!("Failed to save window state: {error}");
    }
}

/// Geometry for a new terminal window from the last saved state, or `None`
/// when nothing usable was saved. Position and window mode are only restored
/// when `restore_position` is set; otherwise the saved size is centered so a
/// second window does not land exactly on top of the first.
pub(crate) fn restored_window_geometry(
    cx: &App,
    min_size: Size<Pixels>,
    restore_position: bool,
) -> Option<RestoredWindowGeometry> {
    Some(load()?.resolve(cx, min_size, restore_position))
}

/// Records `window`'s geometry and writes it once it has stopped changing.
pub(crate) fn schedule_save(window: &Window, cx: &mut App) {
    let Some(state) = SavedWindowState::capture(window, cx) else {
        return;
    };
    let timer = cx.background_executor().timer(SAVE_DELAY);
    let save_task = cx.spawn(async move |cx| {
        timer.await;
        cx.update(flush);
    });
    let writer = cx.default_global::<WindowStateWriter>();
    writer.unsaved = Some(state);
    // Replacing the task cancels the previous, still-waiting save.
    writer.save_task = Some(save_task);
}

/// Records `window`'s geometry and writes it immediately.
pub(crate) fn save_now(window: &Window, cx: &mut App) {
    if let Some(state) = SavedWindowState::capture(window, cx) {
        cx.default_global::<WindowStateWriter>().unsaved = Some(state);
    }
    flush(cx);
}

/// Writes geometry that is still waiting for its delayed save.
pub(crate) fn flush(cx: &mut App) {
    if let Some(state) = cx.default_global::<WindowStateWriter>().unsaved.take() {
        write(&state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
    }

    #[test]
    fn fit_keeps_window_that_is_on_screen() {
        let display = bounds(0.0, 0.0, 1920.0, 1080.0);
        let window = bounds(100.0, 50.0, 1280.0, 820.0);
        assert_eq!(fit_bounds_to_display(window, display), window);
    }

    #[test]
    fn fit_keeps_window_partly_past_the_right_edge() {
        let display = bounds(0.0, 0.0, 1920.0, 1080.0);
        let window = bounds(1700.0, 50.0, 1280.0, 820.0);
        assert_eq!(fit_bounds_to_display(window, display), window);
    }

    #[test]
    fn fit_recenters_window_that_is_off_screen() {
        let display = bounds(0.0, 0.0, 1920.0, 1080.0);
        let window = bounds(2600.0, 50.0, 1280.0, 820.0);
        assert_eq!(
            fit_bounds_to_display(window, display),
            bounds(320.0, 130.0, 1280.0, 820.0)
        );
    }

    #[test]
    fn fit_recenters_window_whose_top_edge_is_unreachable() {
        let display = bounds(0.0, 0.0, 1920.0, 1080.0);
        let above = bounds(100.0, -40.0, 1280.0, 820.0);
        let below = bounds(100.0, 1040.0, 1280.0, 820.0);
        let centered = bounds(320.0, 130.0, 1280.0, 820.0);
        assert_eq!(fit_bounds_to_display(above, display), centered);
        assert_eq!(fit_bounds_to_display(below, display), centered);
    }

    #[test]
    fn fit_shrinks_window_larger_than_the_display() {
        let display = bounds(1920.0, 0.0, 1440.0, 900.0);
        let window = bounds(1920.0, 0.0, 2560.0, 1400.0);
        assert_eq!(
            fit_bounds_to_display(window, display),
            bounds(1920.0, 0.0, 1440.0, 900.0)
        );
    }

    #[test]
    fn saved_state_round_trips_through_json() {
        let state = SavedWindowState {
            mode: WindowMode::Maximized,
            x: 12.0,
            y: 34.0,
            width: 1400.0,
            height: 900.0,
            display_uuid: Some("6c5f8d0e-0000-4000-8000-000000000001".to_string()),
        };
        let json = serde_json::to_string(&state).expect("encode");
        assert!(json.contains("\"mode\":\"maximized\""));
        assert_eq!(
            serde_json::from_str::<SavedWindowState>(&json).expect("decode"),
            state
        );
    }

    #[test]
    fn saved_state_rejects_degenerate_sizes() {
        let mut state = SavedWindowState {
            mode: WindowMode::Windowed,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 800.0,
            display_uuid: None,
        };
        assert!(!state.is_valid());
        state.width = f32::NAN;
        assert!(!state.is_valid());
        state.width = 1200.0;
        assert!(state.is_valid());
    }
}
