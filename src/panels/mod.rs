//! A port of [react-resizable-panels] 4.22.1 (`Group`, `Panel`,
//! `Separator`) to Div.
//!
//! The package's logic is rewritten in Rust on the same web platform
//! features it uses. The layout algorithm ([`layout`]) and the cursor
//! choice ([`cursor`]) are checked against the package's own functions
//! (`tests/panels_oracle.rs`); the DOM, pointer and keyboard behavior
//! ([`group`]) follow `lib/global` and `lib/flex`. Platform features Div
//! lacks are marked `div-gap:` in the code and listed in docs/gaps.md.
//!
//! Not ported yet: the imperative group and panel APIs, double-click reset,
//! resize previews (`resizePreviewMode: "separator"`), `groupResizeBehavior`,
//! persisted and default layouts (`defaultLayout`, `useDefaultLayout`) and
//! `Grid`.
//!
//! [react-resizable-panels]: https://github.com/bvaughn/react-resizable-panels

pub mod cursor;
pub mod group;
pub mod layout;
pub mod number;
pub mod size;
mod stacking;

pub use group::{document_cursor, Group, GroupHandle, MountError, Panel, Separator};
pub use size::Size;

/// A group's resize axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}
