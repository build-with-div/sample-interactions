//! The cursor react-resizable-panels shows while hovering or dragging
//! (`getCursorStyle`), ported for Chromium, where the package's
//! `supportsAdvancedCursorStyles()` is true.

use super::Orientation;

pub const CURSOR_FLAG_HORIZONTAL_MIN: u8 = 0b0001;
pub const CURSOR_FLAG_HORIZONTAL_MAX: u8 = 0b0010;
pub const CURSOR_FLAG_VERTICAL_MIN: u8 = 0b0100;
pub const CURSOR_FLAG_VERTICAL_MAX: u8 = 0b1000;
pub const CURSOR_FLAGS_HORIZONTAL: u8 = 0b0011;
pub const CURSOR_FLAGS_VERTICAL: u8 = 0b1100;

/// The package's interaction state, as far as the cursor cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionState {
    Inactive,
    Hover,
    Active,
}

/// One hit axis: its orientation and the group's `disableCursor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorAxis {
    pub orientation: Orientation,
    pub disable_cursor: bool,
}

/// `getCursorStyle` with `supportsAdvancedCursorStyles() === true`. The
/// result is a CSS `cursor` keyword, or `None` for no override.
pub fn cursor_style(
    cursor_flags: u8,
    axes: &[CursorAxis],
    state: InteractionState,
) -> Option<&'static str> {
    let mut horizontal_count = 0;
    let mut vertical_count = 0;
    if matches!(state, InteractionState::Active | InteractionState::Hover) {
        for axis in axes {
            if axis.disable_cursor {
                continue;
            }
            match axis.orientation {
                Orientation::Horizontal => horizontal_count += 1,
                Orientation::Vertical => vertical_count += 1,
            }
        }
    }
    if horizontal_count == 0 && vertical_count == 0 {
        return None;
    }

    if state == InteractionState::Active && cursor_flags != 0 {
        let horizontal_min = cursor_flags & CURSOR_FLAG_HORIZONTAL_MIN != 0;
        let horizontal_max = cursor_flags & CURSOR_FLAG_HORIZONTAL_MAX != 0;
        let vertical_min = cursor_flags & CURSOR_FLAG_VERTICAL_MIN != 0;
        let vertical_max = cursor_flags & CURSOR_FLAG_VERTICAL_MAX != 0;
        if horizontal_min {
            return Some(if vertical_min {
                "se-resize"
            } else if vertical_max {
                "ne-resize"
            } else {
                "e-resize"
            });
        } else if horizontal_max {
            return Some(if vertical_min {
                "sw-resize"
            } else if vertical_max {
                "nw-resize"
            } else {
                "w-resize"
            });
        } else if vertical_min {
            return Some("s-resize");
        } else if vertical_max {
            return Some("n-resize");
        }
    }

    Some(if horizontal_count > 0 && vertical_count > 0 {
        "move"
    } else if horizontal_count > 0 {
        "ew-resize"
    } else {
        "ns-resize"
    })
}
