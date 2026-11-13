# Gap list

Every `div-gap:` label in this repository, with the web platform feature
that replaces it. Feature numbers are the F-rows of
[design.md](design.md#platform-features-what-div-has). A label marks
temporary scaffolding, never the solution: when the feature lands in Div,
the next bump removes the scaffolding and updates this list.

`Div plan` is the item in Div's platform plan (section 2) that delivers
the feature, or *unscheduled*.

| Label | Where | Web feature | Owner | Size | Div plan |
| --- | --- | --- | --- | --- | --- |
| `div-gap: 24` | `Cargo.toml` | Not a web feature: Div's framework gap 24, a fresh dependency resolve that does not build on Linux. Cargo.lock is seeded from Div's lock | framework | S | unscheduled |
| `div-gap: F17, F13` | `tests/probe_drag.rs` (splitter) | `cursor: col-resize` on the document while dragging; `role="separator"` with `aria-value*` in the accessibility tree | framework + engine; engine | S; M | Tier 1 (1.10, 1.3) |
| `div-gap: F1` | `tests/probe_drag.rs` (splitter) | Implicit pointer capture release after `pointerup`, with `lostpointercapture` | framework + engine | S | Tier 0 (0.12) |
| `div-gap: F3, F1` | `tests/probe_drag.rs` (reorder) | HTML drag and drop with `DataTransfer`; the unified `pointer*` stream | engine + framework per OS; framework | L; M | Tier 3 (I6); Tier 0 (0.12) |

Search with `git grep -n "div-gap"`. A label that is missing here, or a row
whose label is gone from the code, is a review error.
