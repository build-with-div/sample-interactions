//! A port of [TanStack Table] v9's `@tanstack/table-core` 9.3.1: the
//! headless model behind data grids (columns, rows, header groups) with its
//! row sorting, column sizing and column resizing features.
//!
//! table-core is plain JavaScript over row objects, so the port's job is to
//! keep the JavaScript semantics its results depend on: the values and
//! operators of [`value`], the built-in sort functions of [`sort_fns`],
//! V8's `Array.prototype.sort` ([`array_sort`]) and object key order
//! ([`object_map`]). `tests/table_oracle.rs` checks every result against the
//! published package over seeded tables and operations.
//!
//! The rendering (a grid on Div, its header clicks and resize drags) is the
//! next step (M3b in docs/design.md). The model takes events as values:
//! [`Table::toggle_sorting_handler`] for a header click and
//! [`Table::begin_resize`] and [`ResizeSession`] for a drag, including the
//! frame coalescing table-core does with `requestAnimationFrame`.
//!
//! Not ported: the other features (filtering, grouping, expanding,
//! pagination, pinning, visibility, ordering, selection), header families
//! (column pinning), cell and footer APIs, and `manualSorting`.
//!
//! [TanStack Table]: https://github.com/TanStack/table

pub mod array_sort;
mod model;
pub mod object_map;
pub mod sort_fns;
pub mod value;

pub use model::*;
pub use object_map::ObjectMap;
pub use sort_fns::BuiltinSortFn;
pub use value::Value;
