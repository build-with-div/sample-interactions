# Interaction patterns and data views on Div: findings and design

What a professional desktop app needs for direct manipulation, input,
overlays, data views and window chrome, what Div does today, and what Div
should add. This is phase 1 of `sample-interactions`: probes and a plan. The
workbench sample (phase 2) is built only after review.

Pinned Div: `565db46a` (build-with-div/div `main`, 13 November 2026).
The proposed primitives are drafted as an RFC in
[rfc-interactions.md](rfc-interactions.md).

## How this was measured

| Kind | Where | What it shows |
| --- | --- | --- |
| Headless probes | `tests/probe_*.rs` (33 tests) | Each test asserts what Div does today and prints one `PROBE <id>: ...` line. `cargo test -- --nocapture --test-threads=1` reproduces every line quoted below. A probe that fails after a Div bump means a finding changed. |
| Native probe | `src/bin/native_probe.rs`, driven by `tools/native-input.ps1`; output in [evidence/native-input-windows.log](evidence/native-input-windows.log) | A real Win32 window (winit, Div's desktop adapter). The script **posts** mouse messages to it, so the real cursor never moves. Posted messages take the same window procedure as device input, but they are not device input: no OS capture, no pen or touch, and Windows sends `WM_MOUSELEAVE` after each posted move because the real cursor is elsewhere (the `Leave` lines in the log). |
| Source reads | `path:line` at `565db46a` | Where a probe can't reach (pen, touch, file drop, window APIs), the claim cites the code. These are labeled *source*. |

Timings come from the headless probes in the test profile (the app
unoptimized, its dependencies at opt-level 2, as `Cargo.toml` sets), with
the single-threaded test runner, on Windows 11 with 24 logical cores, while
other builds ran on the same machine. Each one is a range over 2–3 runs.
They show orders of magnitude and are not benchmarks.

## Summary

1. **Native wheel scrolling is inverted.** One notch forward (`WM_MOUSEWHEEL`
   +120, which Windows sends for "scroll up") moved a scroller from 500 to
   540, then 580. The horizontal wheel is inverted too. Every Div app on a
   desktop is affected. It is a one-line sign fix in the framework (gap 1).
2. **Drags work today with workarounds.** `Element::set_pointer_capture`
   routes moves to the captor, including outside the window. `hit_test`
   skips a `pointer-events: none` ghost. A splitter, a cross-list reorder
   with a threshold and ghost, and rubber-band selection all pass headless. The
   splitter takes 4.5–7.2 ms per drag move, including render.
3. **The pointer event model is not the web's.** Mouse input fires only
   `mouse*` events and touch fires only `pointer*` events. Capture is never
   released. Right and middle clicks fire `click`. A click goes to the
   release target, so dropping a drag on a button clicks it. There is no
   `dblclick`, click count, `buttons` or pointer type, and native moves
   report button 2. These extend framework gap 4 (gaps 2–7).
4. **Focus primitives are good.** Roving focus by hand,
   `set_modal_root` trap and restore, `focusin`/`focusout` with
   `related_target`, and shortcuts with `Event::key()` all work. The one
   gap: `focus()` does not scroll the element into view (gap 9).
5. **Overlays work as portals.** A `position: fixed` box at the end of
   `<body>` escapes clipping and stacks by `z-index`. Outside-press dismissal
   works with a capture listener and `Element::contains`. Missing are the top
   layer (engine A3, planned), anchor positioning, and any resize
   notification to reposition on (gap 11).
6. **Long lists need windowing.** For mounts 1,000 rows in 0.4–0.7 s and
   5,000 rows in 3.4–5.6 s. A list windowed by hand over 100,000 rows mounts
   in 51–118 ms. A jump that replaces all of its ~26 rows costs 22–37 ms to
   re-window, about 0.9–1.4 ms per row, plus 4–5 ms to render (gap 17).
7. **A style change relays out a lot.** Resizing one column of a 40 × 8 grid
   costs 60–67 ms (block rows) to 88–128 ms (flex rows) of layout and render,
   against 5–6 ms for an unchanged frame (gap 18, engine).
8. **Custom widgets have no accessible state.** The engine reads only
   `aria-label`, `aria-description` and `aria-hidden`. The `role` attribute is
   ignored, and selected, expanded and level cannot be expressed. A grid,
   tree or listbox announces as a bare role at best (gap 12).
9. **OS integration is thin.** File drops reach `App` but are discarded,
   and only a single-window `App` can observe them, with no position.
   Nothing can be dragged out. Windows have no position, state, drag-move or
   custom title bar regions, so tear-off tabs and state restore are not
   possible (gaps 13–15).

## Pattern matrix

Status: **works** (public API, no workaround), **workaround** (doable with a
labeled `div-gap:` workaround in app code), **missing** (cannot be built
without Div changes). Gap numbers refer to [Div gaps](#div-gaps).

### Direct manipulation

| Pattern | Status | Evidence | Gaps |
| --- | --- | --- | --- |
| Drag handle with pointer capture | workaround | `pointer.capture`: moves over a sibling and outside the viewport reach the captor; native confirms negative client coordinates are delivered. winit calls `SetCapture` on press (winit `event_loop.rs:978`, *source*). The app must release capture itself on up. | 3, 7 |
| Drag threshold | workaround | `dnd.cross-list`: a 2 px wiggle stays a click; the app tracks start and distance | — |
| Drag preview / ghost | workaround | `pointer.ghost-hit-test`, `dnd.cross-list`: fixed box with `pointer-events: none`, moved per move | top layer (A3) for z-order guarantees |
| Drop target hit testing | workaround | `Document::hit_test` + parent walk to a `data-*` marker | 5 (release over a button clicks it) |
| Auto-scroll near edges | workaround, not probed | needs a timer while the pointer is still: `div::task::interval` exists (*source*); no per-frame callback | 16 |
| Splitter, keyboard resize | workaround | `split.by-hand`: drag 150 px → 350 px pane, ArrowLeft × 2 → 330 px; 4.5–7.2 ms per move incl. render | 8 (no `col-resize` cursor), 12 (no `aria-valuenow`) |
| Nested split panes | workaround | same as above, nested; nested flex columns hit the known flex cost (sample-git 26) | engine flex fix in progress |
| Sortable list | workaround | `dnd.cross-list` (same code within one list) | 2–5 |
| Kanban cross-list drag | workaround | `dnd.cross-list`: a1 dropped on b2 lands before it | 2–5 |
| Rubber-band selection | workaround | `select.rubber-band`: intersections from `bounding_rect` per move | — |
| In-app DnD API (`draggable`, `drop`) | missing | no drag events or `DataTransfer` in the engine or divui (*source*) | 14 |
| OS files dropped in | missing | `DroppedFile`/`HoveredFile` reach `App::event` and are discarded (`divui/src/app.rs:374-379`); `App::on_platform_event` sees them for a single-window `App` only, with no position | 13 |
| Dragging out to other apps | missing | no drag source in div-platform (*source*) | 13 |

### Input

| Pattern | Status | Evidence | Gaps |
| --- | --- | --- | --- |
| Pointer events (mouse) | workaround | `pointer.mouse-names`: mouse fires `mouse*` only; native mouse is pointer id 0 (`div-platform/src/desktop.rs:971-980`) | 2 |
| Pointer events (touch, pen) | workaround, *source* | touch is pointer id > 0 and fires `pointer*` only, no compatibility `mouse*` (`desktop.rs:1043-1060`); pen arrives as touch; force and tilt are dropped; no pointer type | 2, 7 |
| Wheel scrolling | **missing (broken natively)** | native: a forward notch scrolls down, 500 → 540 → 580; tilt right gives `delta_x` −40 | 1 |
| Touchpad scrolling | missing, *source* | `PixelDelta` goes through the same unsigned mapping (`desktop.rs:1027-1035`) | 1 |
| Pinch zoom | workaround | `input.ctrl-wheel`: Windows precision touchpads send pinch as Ctrl+wheel, and `prevent_default` keeps the scroller still. winit's `PinchGesture` (macOS only) is not forwarded | — |
| Shift+wheel horizontal | missing | `input.shift-wheel`: scrolls vertically | 10 |
| Double click | workaround | `pointer.dblclick`: two `click`s, no `dblclick`, no click count; the app compares timestamps | 6 |
| Right click | workaround | `pointer.right-button`: fires `click` with button 2; no `contextmenu` or `auxclick` (native too) | 4, framework gap 3 |
| Long press | workaround, not probed | pointer down + `div::task::sleep`; touch arrives as `pointer*` only | 2 |
| Context menus | missing | no `contextmenu` event, no popup menu API | framework gap 3 |
| Keyboard shortcuts | works | `keys.shortcut`: `Event::key()` + `modifiers` on `<body>`; `prevent_default` keeps the character out of a focused input | — |
| Command palette | workaround | shortcut + modal root + listbox by hand; the overlay is a portal until A3 | A3 |
| Roving focus | works | `focus.roving` by hand; div-components has `primitives::RovingFocusGroup` | — |
| Focus trap and restore | works | `focus.trap`: `set_modal_root` focuses the first, Tab cycles, outside click keeps focus, `None` restores the opener | nested traps: RFC 0003 request 4 |
| Focus events | works | `focus.events`: `focusin`/`focusout` bubble with `related_target` | — |
| Focus scrolls into view | missing | `focus.scroll-into-view`: `focus()` on row 30 leaves `scroll_top` 0; `scroll_into_view` works | 9 |
| Type-ahead | works | `keys.type-ahead`: `Key::Char` reaches a focused `tabindex` div | — |

### Overlays

| Pattern | Status | Evidence | Gaps |
| --- | --- | --- | --- |
| Escaping a clipping container | workaround | `overlay.portal`: a popover inside `overflow: hidden` is clipped; a `position: fixed` copy mounted on `<body>` at the anchor's rect is drawn | A3 |
| Z-order | workaround | `overlay.z-order`: fixed `z-index: 100` above a `z-index: 10` box, `z-index: 5` below it | A3 |
| Outside-press dismissal | works | `overlay.dismiss`: capture-phase `mousedown` on `<body>` + `Element::contains` | — |
| Repositioning on scroll | workaround | `overlay.reposition-events`: `scroll` arrives at the next frame (`dispatch_pending_events`) | 16 (testing) |
| Repositioning on resize or layout change | missing | no window `resize` event and no element size observer | 11 |
| Anchor positioning (flip, shift, size) | missing | app computes from `bounding_rect` | 11, RFC |
| Tooltip, popover, dropdown, context menu, toast | workaround (portal) | combinations of the above; timers from `div::task` | A3, framework gap 3 |

### Data views

| Pattern | Status | Evidence | Gaps |
| --- | --- | --- | --- |
| Long list, all rows mounted | workaround | `list.unvirtualized`: 1,000 rows 435–671 ms, 5,000 rows 3.4–5.6 s to mount; first render 4–14 ms | 17 |
| Virtualized list (fixed rows) | workaround | `list.windowed`: 100,000 rows, mount 51–118 ms; a 997 px jump (all ~26 rows swapped) 22–37 ms to re-window + 4–5 ms to render | 17 |
| Variable row heights | workaround | `list.measure`: `bounding_rect` heights 20 and 60 px after layout; no notification when they change | 11 |
| Sticky header | works | `grid.sticky-header`: header stays at y 0 after `scroll_to(300)` and is painted there | — |
| Column resize | workaround | `grid.column-resize`: one column of 40 × 8, 1 ms effects + 88–128 ms layout and render (flex rows); 60–67 ms with block rows | 18 |
| Column reorder, sorting | workaround | DnD as above; sorting is app data | 2–5 |
| Selection with Shift/Ctrl | workaround | `Modifiers` on mouse and key events; no shared selection model | RFC |
| Grid keyboard navigation | workaround | key events + `focus()` (roving) | 9 |
| Inline edit | works, not probed separately | `<input>` typing and values work (sample-git) | — |
| Tree with lazy children | workaround | `create_resource` + `Show`/`For` (sample-git patterns) | — |
| Accessibility roles and states | missing | `a11y.custom-widgets`: `set_accessibility_role(Tree)` works; `aria-expanded/selected/level` give `None`; `role="listbox"` gives `GenericContainer` | 12 |

### Window chrome

| Pattern | Status | Evidence | Gaps |
| --- | --- | --- | --- |
| Frameless window | works, *source* | `WindowSpec::decorations(false)` (`divui/src/window.rs:150`) | — |
| Custom title bar drag region | missing | no `drag_window` (winit has it), no non-client hit test (Windows Snap layouts) | 15 |
| Resize from custom edges | missing | no `drag_resize_window` (winit has it) | 15 |
| Docking, tear-off tabs into new windows | missing | `window::open` exists; no window position, no screen pointer coordinates, no cross-window drag | 15, 13 |
| Window state restore | missing | no position, maximized state or move/resize events; no app data directory (sample-git 20) | 15 |
| Cursor shapes | missing | CSS `cursor` has 6 values (`div-style/src/property.rs:296`); the platform maps 5 (`div-platform/src/lib.rs:319-326`) | 8 |

## Div gaps

New gaps found here, numbered for this repository. *Builds on* names the
gap already queued that a fix should extend rather than duplicate. Sizes: S
(a day or two, one PR), M (a week, a few PRs), L (several weeks or
platform work on each OS).

| # | Gap | Evidence | Owner | Size | Builds on |
| --- | --- | --- | --- | --- | --- |
| 1 | Native wheel direction inverted (vertical and horizontal, notch and touchpad); 40 px per notch ignores the system's lines-per-notch | native probe; `div-platform/src/desktop.rs:1027-1040` passes winit's deltas (positive = content moves down, i.e. scroll up) as DOM deltas (positive = scroll down) | framework (div-platform) | S | — |
| 2 | Mouse fires `mouse*` only, touch `pointer*` only; no unified stream | `pointer.mouse-names`, `pointer.pointer-names`; `app.rs` passes `pointer_names = pointer_id != 0` | framework (divui events) | M | framework gap 4 |
| 3 | Pointer capture is never released after up; no `gotpointercapture`/`lostpointercapture`; the cursor ignores the captor | `pointer.capture-release`; native: later presses on `#other` all go to `#handle`; `div-engine/src/interaction.rs:475` keeps the capture on up; `app.rs:471-488` hit-tests for the cursor | engine (interaction) | S | framework gap 4 |
| 4 | Right and middle buttons fire `click`; no `auxclick` | `pointer.right-button`, native | framework (divui events) | S | framework gap 3 (`contextmenu`) |
| 5 | Mouse `click` targets the release element, not the common ancestor of press and release | `pointer.click-common-ancestor`: press a, release b → `click` on b; `divui/src/document.rs:1122` | framework (divui events) | S | framework gap 4 |
| 6 | No `dblclick` and no click count (`detail`) | `pointer.dblclick`, native | framework (divui events) | S | framework gap 4 |
| 7 | `Event` lacks `buttons`, pointer type, pressure, tilt and screen coordinates; native moves report button 2 | `pointer.move-button`, native (`button=2` on every move); `app.rs:289-293` maps `Other(0)` to Right | framework (divui events, div-platform) | S–M | framework gap 4 |
| 8 | Cursor set too small for resize, grab, crosshair, copy, no-drop | `property.rs:296`, `lib.rs:319-326` | engine (style) + framework (platform) | S | — |
| 9 | `focus()` does not scroll the element into view | `focus.scroll-into-view` | engine | S | — |
| 10 | Shift+wheel does not scroll horizontally | `input.shift-wheel` | framework (divui events) or engine | S | — |
| 11 | No resize notification: no window `resize` event, no element size observer | `overlay.reposition-events`, `list.measure` | engine (layout notifications) + framework | M | — |
| 12 | Accessibility: `role` attribute ignored; no selected, expanded, level, set position, row/column index, active descendant, multiselectable or value for custom widgets | `a11y.custom-widgets`; `div-engine/src/accessibility.rs:616,632,679` read only label, description, hidden. div-components' Tabs sets `aria-selected`, which is never exposed | engine (accessibility) | M | RFC 0003 |
| 13 | OS drag and drop: drops discarded, no position or element target, no drag-out | `app.rs:374-379`; native probe prints nothing for drops because only posted mouse input was sent (not probed with a real drag) | framework (div-platform, divui) | M (in) + L (out) | — |
| 14 | No in-app drag and drop API | *source*: no drag events in the engine or divui | components + framework | L | 2–7 |
| 15 | Window geometry and chrome: no position, state, move/resize events, `drag_window`, `drag_resize_window`, non-client regions, screen pointer position, or platform events for `AppSpec` windows | `divui/src/window.rs:97-160`; winit 0.30 has the first five (`winit/src/window.rs:701,734,1522,1537`) | framework (div-platform, window) | M–L | sample-git 20, 21 |
| 16 | Testing: `TestApp` has no drag, wheel, right-click or double-click helpers, and `run_until_idle` does not deliver frame-timed events (`scroll`) | `overlay.reposition-events` needs `dispatch_pending_events` | framework (testing) | S | framework gap 4 |
| 17 | `For` per-row cost: mount superlinear (1,000 → 5,000 rows: 6.4–8.3×), 0.9–1.4 ms per row swapped | `list.unvirtualized`, `list.windowed`; sample-git measured about 1.7 ms per row | framework (renderer) | M | sample-git 8 |
| 18 | A style change relays out much more than it changes: 60–128 ms for one column of 320 cells vs 5–6 ms unchanged | `grid.column-resize`, `grid.column-resize-block` | engine (layout) | M–L, not attributed further | distinct from the nested-flex fix (one flex level here) |

Not counted as new: no top layer, popover or dialog (engine A3), no
`contextmenu` or popup menu (framework gap 3), headless pointer events and
capture (framework gap 4; gaps 2–7 and 16 add detail to it), nested-flex
layout cost (engine, in progress), dynamic style values must be typed
(sample-git 3: `style:left={"300px"}` does not compile; probes call
`set_left(Length::px(..))`), no app data directory (sample-git 20).

## Proposed Div primitives

Full API sketches and semantics are in [rfc-interactions.md](rfc-interactions.md).
Sizes as above.

| # | Primitive | Owner | Size | Needs | Unblocks |
| --- | --- | --- | --- | --- | --- |
| P1 | Pointer event parity: one `pointer*` + compatibility `mouse*` stream with `pointer_type`, `buttons`, `detail`, pressure, screen position; implicit capture release with got/lost events; click to the common ancestor; `auxclick`, `dblclick`, `contextmenu` | framework + engine | M | — | everything that drags |
| P2 | Wheel fix: sign, lines per notch, Shift for horizontal | framework | S | — | every scroller |
| P3 | Cursor set (CSS `cursor` resize/grab/crosshair/copy/no-drop values) and captor's cursor during capture | engine + framework | S | — | splitters, DnD |
| P4 | `TestApp` input helpers: `drag`, `wheel`, `right_click`, `double_click`, `frame()` | framework | S | P1 | testing all of the below |
| P5 | Size observation: `Element::on_resize`, window `resize` event | engine + framework | M | — | P8, P9, P10 |
| P6 | ARIA mapping into the accessibility tree (role, states, relations, values) | engine | M | — | P8, P11–P13, existing Tabs |
| P7 | `Draggable` / `DropTarget` with threshold, ghost, auto-scroll, keyboard drag, and an OS bridge | components (+ framework for OS) | L | P1, P3, A3 | sortable, kanban, column reorder, tear-off |
| P8 | `ResizablePanels` (`PanelGroup`, `Panel`, `PanelHandle`), nested, keyboard, persisted sizes | components | M | P1, P3, P6 | IDE layouts |
| P9 | `VirtualList` with measured variable rows and `scroll_to_index` | components (renderer support from framework) | M | P5, gap 17 | lists, trees, grids |
| P10 | Anchored `Popover` (placement with flip/shift/size), `Tooltip`, `Menu`/`ContextMenu`, `Toaster` on the top layer | components on engine A3 | M each | A3, P5, framework gap 3 | overlays |
| P11 | `SelectionModel<K>` (click, Ctrl, Shift, Ctrl+Shift, keyboard extend, select all) | components | S | — | listbox, tree, grid, rubber band |
| P12 | `TreeView` with lazy children, type-ahead, roving focus | components | M | P6, P9, P11 | file trees |
| P13 | `DataGrid`: sticky header, column resize/reorder, sort, selection, grid keyboard model, inline edit | components | L | P7–P9, P11, gap 18 | tables |
| P14 | Shortcut registry shared with menu accelerators, and a `CommandPalette` | framework (registry, S) + components (palette, M) | S + M | A3 | command-driven apps |
| P15 | Window geometry and chrome: position/state/events, `start_drag_move`, title-bar regions, screen cursor position, per-window platform events | framework | M–L | — | custom chrome, restore, docking |
| P16 | OS drag and drop: element-targeted file drop with position and feedback; drag-out of files and text | framework (div-platform per OS) | M + L | P1 | file managers, editors |

## Suggested order

1. **Now, small, high value.** P2 (wheel sign, S): every app scrolls the
   wrong way. Then the S parts of framework gap 4 as one series: capture
   release (3), click target (5), `auxclick`/`dblclick`/`detail` (4, 6),
   native move button (7), and P4 test helpers. Also P3 cursors (S) and
   `focus()` scrolling into view (9, S).
2. **Event model (P1, M)** while gap 4 is open, so components are written
   once against `pointer*` events.
3. **P6 ARIA mapping (engine, M).** It does not depend on anything above,
   and every data view and the existing Tabs need it.
4. **P11 and P8 (components).** `SelectionModel` (S), then
   `ResizablePanels` (M), the smallest widgets that exercise P1–P3.
5. **P5 observation, then P9 `VirtualList`.** Check gap 17's per-row cost in
   a release build first. If it holds, it is the bottleneck, not layout.
6. **A3 top layer (planned), then P10 overlays (alpha.2 per the plan).**
7. **P7 drag and drop**, then P12 `TreeView` and P13 `DataGrid`. Gap 18
   (layout cost) needs an engine look before a grid of 1,000+ visible cells
   is pleasant.
8. **Platform: P15 window chrome and P16 OS drag and drop.** These need
   per-OS work; file drop in (M) first, drag-out (L) last.

## Phase 2: the workbench (after review)

One app, `sample-interactions`, with a dock-style layout: a file tree (lazy
children from the real file system) on the left, a data grid in the
middle, a properties panel on the right, a kanban board tab, and files
dropped in from the OS. It is built in small PRs, each using only Div's
public API and `// div-gap: <n>` workarounds, with gaps recorded in
`docs/framework-gaps.md` as they are hit:

1. Split panes (nested, keyboard, sizes saved to a file in the working
   directory until an app data dir exists).
2. File tree (lazy, keyboard, type-ahead, selection).
3. Windowed data grid (sticky header, sort, column resize, selection,
   keyboard, inline edit) over generated rows.
4. Kanban board with cross-list drag, ghost and auto-scroll.
5. Command palette (Ctrl+K) and a context menu drawn as a portal.
6. Files dropped in, through `App::on_platform_event`, which needs a
   single-window `App` (gap 13).

## Reproduce

Build environment as in Div's CI (see the README). Then:

```sh
cargo test -- --nocapture --test-threads=1        # every PROBE line
cargo build --bin native_probe
pwsh tools/native-input.ps1                        # Windows only; opens a window for ~3 s
```
