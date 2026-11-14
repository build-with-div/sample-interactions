# Gap list

Every `div-gap:` label in this repository, with the web platform feature
that replaces it. Feature numbers are the F-rows of
[design.md](design.md#platform-features-what-div-has). A label marks
temporary scaffolding, never the solution: when the feature lands in Div,
the next bump removes the scaffolding and updates this list.

`Div plan` is the item in Div's platform plan (section 2) that delivers
the feature, or *unscheduled*.

## In the ports

`src/panels/group.rs` (react-resizable-panels):

| Feature | What the package uses | What the port does until then | Owner | Size | Div plan |
| --- | --- | --- | --- | --- | --- |
| F1 unified Pointer Events | `pointerdown/move/up/leave` for every pointer | Also listens to `mouse*`, which is all Div sends for the mouse | framework | M | Tier 0 (0.12) |
| F1 `pointerType` | Ignores non-primary mouse buttons only for `"mouse"` | Infers the mouse from the `mouse*` event name | framework | S | Tier 0 (0.12) |
| F1 `buttons` | Ends a drag when a move reports no buttons (a missed release) | Skips the check | framework | S | Tier 0 (0.12) |
| F1 `hasPointerCapture` | Captures on the first move unless already captured | Captures once per drag | framework + engine | S | Tier 0 (0.12) |
| F1 implicit capture release | Capture ends after `pointerup` | Releases capture explicitly on release | framework + engine | S | Tier 0 (0.12) |
| F1 `movementX`/`movementY` | Keeps the cursor flags of an axis with no movement | Treats every event as movement | framework | S | Tier 0 (0.12) |
| F1 `click` after a cancelled up | Cancels the `pointerup` that ends a drag; Chromium still fires `click` | Does not cancel Div's `mouseup`, which would drop the `click` (`divui/src/document.rs:1108`) | framework | S | Tier 1 (1.1, I2) |
| F1 `dblclick` | Resets a panel to its default size | Not wired: needs the imperative API (next PR) and Div's `dblclick` | framework | S | Tier 1 (1.1, I2) |
| F4 `<dialog>`, `showModal()` | Rejects pointer targets inside a modal dialog that does not contain the group | Nothing to check: Div has no dialog | engine | L | scheduled (engine lane, top layer and `<dialog>`) |
| F8 `ResizeObserver` | Re-derives pixel constraints whenever the group resizes | Re-derives when a pointer or key interaction starts | framework + engine | M | Tier 0 (0.11) |
| F10 `offsetWidth`, `offsetLeft` | Measures and orders panels with offset geometry | Uses the bounding rect, equal without transforms | engine | S | Tier 2 (2.10, I4) |
| F12 `focus({ preventScroll, focusVisible })` | Focuses separators without scrolling or a focus ring | `focus()`, which never scrolls in Div; no `focusVisible` | engine | S | Tier 2 (2.10, I4) |
| F13 ARIA `role` | `role="separator"` with `aria-value*` | Sets the attributes; Div's accessibility tree ignores `role` | engine | M | Tier 1 (1.3) |
| F16 `touch-action` | `pan-y`/`pan-x` on the group and panels, `none` on separators | Not set: Div has no `touch-action` | engine + framework | M | Tier 2 (2.10, S15) |
| F17 cursor keywords; runtime stylesheet | Adopts `*, *:hover { cursor: ew-resize !important }` | Computes the package's cursor (`panels::document_cursor`) but cannot apply it | engine + framework | S; L | Tier 1 (1.10); decision 2 (later) |
| F19 window `blur`, `pagehide` | End an active drag | Not handled | framework | M | Tier 3 (I7) |
| F20 `matchMedia("(pointer: coarse)")` | 20px hit targets on coarse pointers, 10px on fine | Always 10px | framework + engine | S | Tier 1 (1.2) |

## Measured by the fidelity runner

Differences between the port on Div and the package in pinned Chromium
that need no scaffolding, because the port does what the package does and
Div gives a different result. `tests/fidelity_panels.rs` holds them to
[panels-div-differences.txt](../tests/fixtures/fidelity/panels-div-differences.txt),
value for value.

| Feature | Difference | Count | Probe without the port | Owner | Size | Div plan |
| --- | --- | --- | --- | --- | --- | --- |
| F21 flex layout with fractional `flex-grow` | Panel and separator boxes land one LayoutUnit (1/64 px) from Chromium's | 99 of 380 boxes | `layout.flex-grow-fractions` (`tests/probe_layout.rs`) | engine | S (estimate) | unscheduled |
| F1 click target | A press on one element released over another: Chromium clicks their nearest common ancestor, Div the release element | 1 of 69 snapshots | `pointer.click-common-ancestor` | framework + engine | S | Tier 1 (1.1, I2) |

## In the probes

| Label | Where | Web feature | Owner | Size | Div plan |
| --- | --- | --- | --- | --- | --- |
| `div-gap: F17, F13` | `tests/probe_drag.rs` (splitter) | `cursor: col-resize` on the document while dragging; `role="separator"` with `aria-value*` in the accessibility tree | framework + engine; engine | S; M | Tier 1 (1.10, 1.3) |
| `div-gap: F1` | `tests/probe_drag.rs` (splitter) | Implicit pointer capture release after `pointerup`, with `lostpointercapture` | framework + engine | S | Tier 0 (0.12) |
| `div-gap: F3, F1` | `tests/probe_drag.rs` (reorder) | HTML drag and drop with `DataTransfer`; the unified `pointer*` stream | engine + framework per OS; framework | L; M | Tier 3 (I6); Tier 0 (0.12) |

## Build

| Label | Where | What | Owner | Size | Div plan |
| --- | --- | --- | --- | --- | --- |
| `div-gap: 24` | `Cargo.toml` | Not a web feature: Div's framework gap 24, a fresh dependency resolve that does not build on Linux. Cargo.lock is seeded from Div's lock | framework | S | unscheduled |

Search with `git grep -n "div-gap"`. A label missing here, or a row whose
label is gone from the code, is a review error.
