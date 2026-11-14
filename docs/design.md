# Interaction patterns and data views on Div: the platform they need

This sample ports the npm packages that professional desktop apps use for
direct manipulation, overlays and data views, so that Div can run their
logic in Rust. The sample's main deliverable is the list of **web platform
features** those packages depend on, which Div must implement faithfully:
matching Chromium 147's behavior and pixels, and measured against it. Div
never runs JavaScript, so a port is the package's logic rewritten in Rust,
on top of the same platform features the original uses.

Phase 1 (this document) names the packages, maps them to platform features,
and probes what Div has today. Phase 2 builds the ports in this repository,
compared against the original packages running in pinned Chromium.

Pinned Div: `92ea25a0` (build-with-div/div `main`, 13 November 2026). The
source reads below were taken at `565db46a`; the two commits between them
change only how `view!` shows `{expr}` children, and the probes pass
unchanged at both.
[rfc-interactions.md](rfc-interactions.md) drafts the platform features as
a Div RFC.

## Packages and the platform under them

The platform features below each package come from its source and
documentation. Each row is checked against the pinned versions when its
reference page is built (see [Fidelity plan](#fidelity-plan)).

| Package | What it is for | Platform features it depends on |
| --- | --- | --- |
| **dnd-kit** (`@dnd-kit/core`, `/sortable`) | sortable lists, kanban, any drag | Pointer Events on the document (`pointerdown/move/up/cancel`, `button`, `isPrimary`); activation by distance or delay (timers); `getBoundingClientRect`; scroll containers found from computed `overflow`, with `scrollTop`/`scrollLeft`, `scrollBy` and `scroll` events; auto-scroll on an interval; `transform: translate3d` on the dragged item; `DragOverlay` with `position: fixed`; `ResizeObserver` for droppables; `touch-action: none` on handles; `user-select: none` while dragging; Escape to cancel; keyboard sensor (`KeyboardEvent.key`); `aria-live` announcements and `aria-describedby` |
| **react-dnd** (HTML5 backend) | drag and drop, including files from the OS | The HTML drag-and-drop API: `draggable`, `dragstart/drag/dragend/dragenter/dragover/dragleave/drop`, and `DataTransfer` (`setData`, `types`, `files`, `items`, `effectAllowed`, `dropEffect`, `setDragImage`). OS file drops arrive as `DataTransfer.files` |
| **react-resizable-panels** | split panes and splitters | Pointer Events on the handle with document-level move and up; `flex-grow` sizing; `ResizeObserver` on the group; `cursor: col-resize/row-resize/ew-resize/ns-resize` set on the document while dragging; `user-select: none`; `touch-action: none`; keyboard (arrows, Home, End, Enter); `role="separator"` with `aria-valuenow/min/max` and `aria-controls`; `data-*` attributes for styling |
| **Radix UI primitives** | dialog, popover, menu, context menu, tooltip, roving focus | `tabindex`, `focus({preventScroll})`, `focusin`/`focusout`; `KeyboardEvent.key`; `pointerdown` and `pointermove` (submenu grace area); `contextmenu`, plus long press on touch; portals into `<body>`; outside-pointer blocking through `pointer-events: none` on `<body>`; hiding siblings with `aria-hidden` (what `inert` does natively); scroll lock (`overflow: hidden`, wheel and touch `preventDefault`); `animationstart`/`animationend` and computed `animation-name` for exit animations; CSS custom properties (`--radix-*`); `data-state` attribute selectors; `ResizeObserver`; tree-order comparison; timers for typeahead and tooltip delays |
| **Floating UI** | anchored placement under Radix popovers, menus and tooltips | `getBoundingClientRect`, `getClientRects`, computed style for containing blocks, scroll ancestors and their `scroll` events, window `resize`, `ResizeObserver`, `IntersectionObserver` (layout shift), `requestAnimationFrame`, viewport size. The platform's native equivalent is **CSS anchor positioning** plus the **`popover` attribute** and the **top layer** |
| **cmdk** | command palette | `input` and `keydown`; `scrollIntoView({block: "nearest"})`; `aria-activedescendant`, `role="combobox/listbox/option"`, `aria-selected`, `aria-expanded`; `data-selected` selectors; `ResizeObserver` writing `--cmdk-list-height` with a CSS `height` transition; a Radix `Dialog` for the palette |
| **TanStack Virtual** (`virtual-core`) | virtualized lists and grids | `scroll` events, `scrollTop`/`scrollLeft`, `scrollTo` with `behavior`; `scrollend`; `ResizeObserver` per measured item (`borderBoxSize`); `getBoundingClientRect`; absolutely positioned items with `transform` |
| **TanStack Table** (`table-core`) | headless data grid: sorting, filtering, selection, column sizing, ordering, pinning | Mostly logic. Column resize uses mouse and touch events on the document. Rendering needs table or grid layout, `position: sticky` for pinned headers and columns, and `user-select` |
| **AG Grid** (Community) | full data grid; fidelity reference for a grid | Absolutely positioned rows and cells, synchronized `scroll` between viewports, `ResizeObserver`, mouse, pointer and touch events, focus management (`tabindex`, `focus({preventScroll})`), the HTML drag-and-drop API for row drag, clipboard, `role="grid/treegrid"` with `aria-rowindex/colindex/rowcount/colcount`, `user-select`, `requestAnimationFrame`, CSS custom properties for themes |
| **react-arborist** | tree view | react-window (`scroll` events, absolute positioning), react-dnd's HTML5 backend, `tabindex` and `keydown`, `role="tree/treeitem"` with `aria-level`, `aria-expanded` and `aria-selected`, and `ResizeObserver` for auto-sizing |

Window chrome has no package in this list; desktop apps get it from their
shell. The closest web-platform features are Chromium's `app-region: drag`
CSS property (used by Electron, and by installed web apps with Window
Controls Overlay) and the window geometry APIs (`screenX`, `moveTo`,
`resizeTo`).

## Platform features: what Div has

Status at `92ea25a0`: **has** (matches what the probes check), **partial**,
or **missing**. Evidence is a probe id from `tests/probe_*.rs`, the native
probe log, or *source* (a code read, cited as `path:line`). Sizes: S (one
PR), M (a few PRs), L (several weeks, or per-OS work). The packages column
says who needs each feature.

| # | Platform feature | Div today | Missing | Owner | Size | Packages |
| --- | --- | --- | --- | --- | --- | --- |
| F1 | **Pointer Events** (Level 3) and the UI Events mouse model | partial. `set_pointer_capture` routes moves to the captor, including outside the window (`pointer.capture`, native). Touch fires `pointer*`. `hit_test` skips `pointer-events: none` (`pointer.ghost-hit-test`) | Mouse fires only `mouse*` and touch only `pointer*`, where Chromium sends `pointer*` and then compatibility `mouse*`. Capture is never released after up, and there are no `gotpointercapture`/`lostpointercapture` events (`pointer.capture-release`, native). Right and middle buttons fire `click`, and there is no `auxclick` (`pointer.right-button`). `click` targets the release element, not the common ancestor (`pointer.click-common-ancestor`). No `dblclick` or `detail` (`pointer.dblclick`). No `pointerType`, `buttons`, `isPrimary`, pressure, tilt or screen coordinates. Native moves report button 2 (native log; `divui/src/app.rs:289-293`). A cancelled `mouseup` or `pointerup` suppresses `click` (`pointer.click-after-up`; `divui/src/document.rs:1108`); Chromium still fires it. No `hasPointerCapture` or `movementX`/`movementY`. No `contextmenu` (framework gap 3). | framework (divui events, div-platform) + engine (`interaction.rs` capture, click target) | M | all |
| F2 | **Wheel events** (UI Events `WheelEvent`) | partial. Cancellable `wheel` events; Ctrl+wheel `preventDefault` works (`input.ctrl-wheel`) | **Native direction is inverted**: one notch forward scrolled 500 → 540 → 580, and horizontal is inverted too (native log; `div-platform/src/desktop.rs:1027-1040`). 40 px per notch, ignoring the system's lines per notch. No `deltaMode`. Shift+wheel does not scroll horizontally (`input.shift-wheel`) | framework (div-platform) | S | all scrolling |
| F3 | **HTML drag and drop** and **`DataTransfer`**, including OS files in and out | missing. No `draggable` or drag events (*source*). OS drops reach `App` and are discarded (`divui/src/app.rs:374-379`); only a single-window `App` can observe them, through `on_platform_event`, with no position | The whole API: the drag event sequence, `DataTransfer` (one type shared with the clipboard, files as `File` objects wrapping paths: decision 7), the drag image, drop effects, and OS bridges (Windows OLE `IDropTarget`/`DoDragDrop`, X11 XDND, Wayland data device) | engine (events, defaults, drag image) + framework (div-platform per OS) | L | react-dnd, react-arborist, AG Grid |
| F4 | **Top layer**, **`<dialog>`** (`show`, `showModal`, `::backdrop`, Escape) | partial. `Document::set_modal_root` contains Tab and restores focus (`focus.trap`). Portals by hand: a fixed box at the end of `<body>` escapes clipping and stacks by `z-index` (`overlay.portal`, `overlay.z-order`) | No `<dialog>` element (`div-dom` `ElementTag`), no top layer, no `::backdrop` | engine (A3, planned) | L (A3) | Radix, cmdk |
| F5 | **`popover` attribute** (`auto`, `manual`, `hint`; light dismiss; `showPopover`; `popovertarget`; `toggle` events) | missing (*source*) | All of it. Light dismiss today is a capture listener plus `Element::contains` (`overlay.dismiss`) | engine (on F4) | M | Radix, Floating UI |
| F6 | **CSS anchor positioning** | partial. `anchor-name`, `position-anchor` and `position-area` exist, with 9 `position-area` values, for anchors laid out in the same containing block (`div-layout/src/block.rs:58-75`, `div-style/src/computed.rs:163`) | `anchor()` and `anchor-size()`, `position-try-fallbacks`, `position-try-order`, `@position-try`, `position-visibility`, the `span-*` area keywords, anchors in other containing blocks and in the top layer, and the implicit anchor from `popovertarget` | engine (style, layout) | M–L | Floating UI's job, natively |
| F7 | **`inert`** | missing (*source*). The modal root covers the focus half of one modal | The attribute: no focus, no hit testing, hidden from accessibility | engine | S–M | Radix (`hideOthers`) |
| F8 | **`ResizeObserver`** and the window `resize` event | missing (`overlay.reposition-events`, `list.measure`) | One observer with the three boxes (`border-box`, `content-box`, `device-pixel-content-box`): the engine computes after layout and the framework delivers before paint (decision 4); the window `resize` event | engine (layout notifications) + framework (delivery) | M | dnd-kit, panels, Radix, Floating UI, cmdk, TanStack Virtual, AG Grid, react-arborist |
| F9 | **`IntersectionObserver`** | missing (*source*) | Observer with roots, margins and thresholds | engine | M | Floating UI, infinite lists |
| F10 | **Geometry** (`getBoundingClientRect`, `getClientRects`, `client*`, `scroll*`, `elementFromPoint`, `getComputedStyle`) | has: `bounding_rect`, `client_rects`, `client_width`, `scroll_height`, `hit_test` and `computed_style` (`list.measure`, `select.rubber-band`) | `elementsFromPoint`, `offsetParent`/`offsetTop`, `visualViewport` | engine | S | all |
| F11 | **Scroll APIs and events** | partial. `scroll_to`, `scroll_by`, smooth scrolling and `scroll_into_view` with options; `scroll` is delivered at frame time (`overlay.reposition-events`); scroll snap | `scrollend`, `overscroll-behavior`, scroll anchoring (`overflow-anchor`); `TestApp` does not deliver frame-timed events (gap T1 below) | engine | S–M | TanStack Virtual, dnd-kit, AG Grid |
| F12 | **Focus** (`tabindex`, `focus()`, `focusin`/`focusout`, `relatedTarget`, `activeElement`) | has (`focus.roving`, `focus.events`, `focus.non-focusable`, `keys.shortcut`, `keys.type-ahead`) | `focus()` does not scroll into view (`focus.scroll-into-view`), and there is no `preventScroll` option. `:focus-visible` is in the engine track's pseudo-class drafts | engine | S | Radix, cmdk, grids |
| F13 | **ARIA in the accessibility tree** | partial. `set_accessibility_role` works for Tree (`a11y.custom-widgets`) | The `role` attribute is ignored (`role="listbox"` → `GenericContainer`). Only `aria-label`, `aria-description` and `aria-hidden` are read (`div-engine/src/accessibility.rs:616,632,679`). No `aria-selected/expanded/level/posinset/setsize/rowindex/colindex/activedescendant/valuenow/live`. div-components' Tabs sets `aria-selected`, which is never exposed | engine (accessibility) | M | all |
| F14 | **`position: sticky`** | has (`grid.sticky-header`) | — (pixel comparison in grid fixtures) | engine | — | TanStack Table, AG Grid |
| F15 | **`user-select`** | missing (*source*); whether Div selects static text on a drag is not probed | The property, and text selection behavior on drag | engine | S–M | dnd-kit, panels, grids |
| F16 | **`touch-action`** and touch panning | missing (*source*) | The property, panning and zooming by touch, and `pointercancel` when panning takes over | engine + framework | M | dnd-kit, panels |
| F17 | **CSS `cursor`** keywords | partial: 6 values (`div-style/src/property.rs:296`, `div-platform/src/lib.rs:319-326`); the cursor comes from the hit element, not the captor (`app.rs:471-488`) | `col-resize`, `row-resize`, `ew/ns/nesw/nwse-resize`, `grab`, `grabbing`, `crosshair`, `copy`, `alias`, `no-drop`, and the rest of the keyword set | engine (style) + framework (platform) | S | panels, dnd-kit, AG Grid |
| F18 | **Frame callback** (`requestAnimationFrame`) and timers | partial. `div::task::sleep` and `interval` exist (*source*) | A per-frame callback before style and layout | framework | S | Floating UI, dnd-kit, AG Grid |
| F19 | **`app-region: drag`** and window geometry (`screenX/Y`, `moveTo`, `resizeTo`, maximize state, `resize`/`move` events) | partial. `WindowSpec` has title, size, min, max, resizable and decorations (`divui/src/window.rs:97-160`) | Drag regions, window position and state, move and resize events, the pointer's screen position, and platform events for `AppSpec` windows. winit 0.30 has `drag_window`, `drag_resize_window` and `outer_position` | framework (window, div-platform) | M–L | custom chrome, tear-off, state restore |
| F20 | **Media queries from script** (`matchMedia`, the `pointer` and `hover` media features) | missing (*source*) | `matchMedia` with change events, and `(pointer: coarse)`/`(hover: none)` from the input devices present | framework + engine | S | react-resizable-panels (coarse hit targets), Radix |

Already queued in Div, and needed by the same packages: CSS custom
properties, attribute selectors and `data-state` styling (Tier 0 items 0.1
and 0.5), `contextmenu` (framework gap 3), and the top layer (engine A3).
Radix and cmdk also write custom properties at runtime
(`--radix-popover-content-transform-origin`, `--cmdk-list-height` through
`style.setProperty`); decision 2 adopts runtime custom-property writes
(engine, with the framework's API). Not probed here.

The platform design decisions of 13 November 2026 (Div's platform plan,
`decisions-2026-11-13.md`) settle this design's open API questions:
Pointer Events use typed enums and bitflags carrying web values, with mouse
`pointerId` 1 as in Chromium; `requestAnimationFrame` is per document with
a typed handle; `ResizeObserver` is one API with three boxes;
`DataTransfer` is one type for clipboard and drag, with `File` objects; and
window chrome uses Chromium's `app-region`. The RFC draft records them.

### Costs that the ports will hit

These are not missing features, but they decide whether ports of TanStack
Virtual and Table are usable:

- **Mounting rows (framework).** `For` mounts 1,000 rows in 435–671 ms and
  5,000 rows in 3.4–5.6 s (`list.unvirtualized`). A list windowed over
  100,000 rows, the shape TanStack Virtual renders, mounts in 51–118 ms. A
  jump that replaces its ~26 rows costs 22–37 ms to re-window, about
  0.9–1.4 ms per row, plus 4–5 ms to render (`list.windowed`). sample-git
  measured about 1.7 ms per row (its gap 8).
- **Relayout after a style change (engine, not attributed further).**
  Resizing one column of a 40 × 8 grid costs 1 ms of effects plus 88–128 ms
  of layout and render with flex rows, or 60–67 ms with block rows, against
  5–6 ms for an unchanged frame (`grid.column-resize`,
  `grid.column-resize-block`). This is a single flex level, so it is
  distinct from the nested-flex fix in progress.

Timings are from the headless probes in the test profile (the app
unoptimized, dependencies at opt-level 2), with a single-threaded test
runner, on Windows 11 with 24 logical cores, while other builds ran on the
machine. Each is a range over 2–3 runs. They are not benchmarks.

### Testing gaps

| # | Gap | Owner | Size |
| --- | --- | --- | --- |
| T1 | `TestApp` has no pointer, drag, wheel, right-click or double-click helpers. `run_until_idle` does not deliver frame-timed events: `scroll` needs `dispatch_pending_events` | framework (testing) | S |
| T2 | Div's behavior-trace harness (`tests/fidelity/components`) drives Chromium with keys and clicks only. The ports need pointer down, move and up with `buttons`, wheel, drag (`Input.dispatchDragEvent`) and touch steps, and geometry and accessibility-tree comparison per step | components (harness) | S–M |

## Fidelity plan

Every port is compared with the original package in pinned Chromium 147,
with the same input on both sides. This follows Div's existing
behavior-trace harness (`tests/fidelity/components`). There, a Radix page in
Chromium, driven through CDP, and the same fixture in Div are compared
after each input step.

1. **Reference pages.** `fidelity/reference/` holds one small page per
   fixture, built with the real npm packages at versions pinned in
   `package-lock.json`. Examples: a dnd-kit sortable list, a 3-panel
   react-resizable-panels layout, a Radix popover anchored near the
   viewport edge, a cmdk palette, a TanStack Virtual list of 10,000
   variable rows, a TanStack Table with sorting and resizing, and a
   react-arborist tree. Styles are fixed: the same fonts and no system UI.
2. **The same fixture in Div,** built from the Rust port, with the same
   `data-slot` names.
3. **Input scripts** in `fidelity/traces.json`. They use Div's step kinds
   (`press`, `click`, `type`) and add `pointer` (down, move or up at a
   point, with `buttons`), `drag` (a sequence of moves), `wheel`, `touch`
   and `drop-files`. Chromium receives them through CDP
   `Input.dispatchMouseEvent`, `dispatchDragEvent` and `dispatchTouchEvent`.
   Div receives them through `Document` dispatch, the same calls the native
   adapter makes.
4. **Compared after each step:**
   - the attributes and focus trace (as Div's harness does);
   - the geometry of every slot (panel sizes after a drag, popover position
     after a flip, virtual row offsets after a scroll);
   - the accessibility tree: Chromium's `Accessibility.getFullAXTree`
     against Div's `accessibility_update`, comparing roles, states and
     relations;
   - pixels, with Div's `pixel-compare`.
5. **Platform features themselves** are measured with WPT where Div's WPT
   tooling can run them: `pointerevents`, `uievents` (wheel),
   `html/editing/dnd`, `html/semantics/interactive-elements/the-dialog-element`,
   `html/semantics/popovers`, `inert`, `css/css-anchor-position`,
   `resize-observer`, `intersection-observer`, `css/css-position/sticky`,
   `css/css-ui` (`cursor`, `user-select`), and `core-aam`/`html-aam` for the
   accessibility mapping. Interaction tests rely on testdriver Actions, so
   they need T1 and T2 first.

Phase 2 adds the reference pages and the trace runner here. A change to
Div's harness (T2) goes to the components track as a proposal.

## What the probes show today

The probes are evidence for the table above. They assert what Div does at
`92ea25a0` and print a `PROBE <id>: ...` line. A probe that starts failing
after a Div bump means a finding changed.

| Kind | Where | Notes |
| --- | --- | --- |
| Headless | `tests/probe_*.rs` (34 tests) | `cargo test -- --nocapture --test-threads=1` reproduces every probe id quoted here |
| Native, Windows | `src/bin/native_probe.rs` and `tools/native-input.ps1`; output in [evidence/native-input-windows.log](evidence/native-input-windows.log) | A real Win32 window. The script **posts** mouse messages, so the real cursor never moves. Posted messages take the same window procedure as device input but are not device input: no OS capture, no pen or touch. The `Leave` after each posted move is Windows reporting the real cursor outside the window |
| Source | `path:line` at `565db46a` | Pen, touch, file drop and window APIs, where a probe can't reach |

`tests/probe_drag.rs` builds a splitter, a cross-list reorder with a
threshold and ghost, and rubber-band selection on today's API. They pass,
and the splitter takes 4.5–7.2 ms per drag move including render. They are
evidence of which pieces exist (capture routing, `hit_test`, geometry), not
a proposal: each shortcut they take carries a `div-gap:` comment naming the
platform feature that replaces it.

## Workarounds in the sample

A sample may use a workaround only to keep building while the platform
feature is missing. It is labeled `// div-gap: F<n>` in code and logged in
`docs/framework-gaps.md` with the feature that replaces it. A workaround is
never the proposed solution, and the sample does not reimplement a platform
feature privately. For example, until F8 lands, a port that needs
`ResizeObserver` re-measures with `bounding_rect` after its own changes and
says so. It does not grow an observer of its own.

## Suggested order for Div

1. **F2 wheel direction (S).** Every Div app scrolls the wrong way natively.
2. **F1 in two steps.** First the S fixes: capture release with
   got/lost events, the click target, `auxclick`, `dblclick` and `detail`,
   the native move button, and `buttons` and `pointerType`, together with
   T1. Then the unified `pointer*` + compatibility `mouse*` stream (M). All
   of this extends framework gap 4.
3. **F13 ARIA mapping (M).** Independent of the rest, and the only way the
   tree, grid, listbox and existing Tabs are accessible.
4. **F8 `ResizeObserver` and F17 cursors.** Most packages need F8; F17 is
   S.
5. **F4 top layer and `<dialog>` (A3, planned), then F5 `popover`, F7
   `inert` and F6 anchor positioning.** Together these replace Floating
   UI's JavaScript with the platform's own placement.
6. **F11 and F12 scroll and focus details, F15, F16, F18 (S each).**
7. **F3 HTML drag and drop:** in-app first (M), then OS files in (M), then
   drag-out (L) per OS. **F9 `IntersectionObserver` (M).**
8. **F19 window chrome (M–L).**

Not yet attributed: the mount and relayout costs above go to the framework
and engine tracks as measurements, with the probes to reproduce them.

## Phase 2: the ports

Div scheduled its Tier 0 foundations on 13 November 2026 (see the
platform plan, section 2). For this sample that covers F2 wheel direction
(0.9), F18 `requestAnimationFrame` with frame advance (0.10), F8
`ResizeObserver` (0.11), and the unified F1 Pointer Events with T1 input
helpers (0.12). F13 ARIA mapping, the rest of F1 (`dblclick`, `auxclick`,
the click target) and F17 cursors are Tier 1. F4 to F7 overlays are Tier 2
and F3 drag and drop is Tier 3; none of these are scheduled.

So the ports start with the pieces that run on today's Div. Every missing
feature is scaffolded only as `// div-gap: F<n> <web feature>` at the point
where the platform feature belongs. [gaps.md](gaps.md) lists each label
with its file, and the gap is reported to the parent session. When a
feature lands in Div, its scaffolding is removed in the next bump, and the
probe that recorded the old behavior is updated in the same PR.

One PR per milestone, merged when CI is green:

| # | Milestone | Package | Runs on today's Div | Scaffolded (`div-gap`) |
| --- | --- | --- | --- | --- |
| M0 | Pin Div `92ea25a0`; this plan | — | — | — |
| M1 | Panel group layout model and its view: sizes in percent, min, max, collapsible, nested groups, keyboard resizing on the separator | react-resizable-panels | pointer capture, flex sizing, keyboard events, `aria-value*` attributes | F1 capture release and document-level up; F8 group size; F13 `role="separator"`; F15 `user-select`; F17 `col-resize` cursor |
| M2 | Fidelity runner and the first reference page: the same scripted drag and key steps in pinned Chromium (CDP) and in Div, comparing slot geometry and attributes after each step | react-resizable-panels | `Document` dispatch, `bounding_rect` | T2 step kinds live here until Div's harness has them |
| M3 | Table model: column definitions, sorting, column sizing (`onChange` and `onEnd` modes), the row model, rendered as a grid | TanStack Table (`table-core`) | sticky header (F14), pointer capture | F13 `role="grid"`; F17 cursor; F1 |
| M4 | Virtualizer: measurement, overscan, `scrollToIndex`, combined with M3 | TanStack Virtual (`virtual-core`) | scroll APIs, frame-timed scroll events | F8 row measurement; F11 `scrollend` |
| M5 | Drag sensors, collision detection, sortable lists and kanban, auto-scroll | dnd-kit core and sortable | pointer capture, `hit_test`, geometry, timers | F1 unified stream; F15; F16 `touch-action`; F18 |
| M6 | Tree with keyboard navigation, multi-select and in-app drag reorder | react-arborist | as M4 and M5 | F13 `role="tree"`; F3 drag and drop |
| M7 | Command palette on div-components' dialog | cmdk | modal root, focus trap, keyboard | F4 `<dialog>`; F13 `role="listbox"` |
| M8 | Workbench app combining the ports in one window | — | `App`, menus | F19 window chrome; F3 OS file drop |

### M1 status: react-resizable-panels

`src/panels` ports react-resizable-panels 4.22.1 (commit `71c72536`):
`Group`, `Panel` and `Separator` with sizes in px, %, rem, em, vh and vw;
min, max and collapsible panels with thresholds; nested groups; the
document-level pointer handling with hit regions, focus, click
suppression and pointer capture; separator keys (arrows, Home, End,
Enter, F6); the separator's ARIA values and `data-*` state; and the
package's cursor choice. Not yet ported: the imperative APIs,
double-click reset, resize previews, `groupResizeBehavior`, persisted and
default layouts, and `Grid`.

Verification, on Windows with Div `92ea25a0`:

- `cargo test --test panels_oracle`: the port's layout and cursor
  functions against the package's own, run in Node 22.18 over seeded
  inputs (`tools/rrp-oracle/generate.mjs`). All 10,684 cases are
  bit-for-bit equal: `formatLayoutNumber` 1,000, `validateItemSize`
  1,000, `adjustLayoutByDelta` 6,000 (2,000 inputs × 3 triggers),
  `validateAxisLayout` 1,000, `calculateDefaultLayout` 300,
  `calculateSeparatorAriaValues` 1,000 and `getCursorStyle` 384.
- `cargo test --test panels_group`: 10 tests of the DOM and interactions
  on Div, driven by `Document` dispatch. Panel widths follow the layout
  exactly (198, 396 and 198 px for 25/50/25 of 792 px).

This is not yet a comparison with the package running in Chromium; that
is M2. The port surfaced one new Div gap: Div drops the `click` after a
cancelled `mouseup` or `pointerup` (F1).

M2 to M7 each add the package's reference page and trace steps to the
fidelity runner, and report measured differences per step. A milestone
whose platform feature lands in Div first is built on the feature instead
of scaffolding.

## Reproduce

Build environment as in Div's CI (see the README). Then:

```sh
cargo test -- --nocapture --test-threads=1        # every PROBE line
cargo build --bin native_probe
pwsh tools/native-input.ps1                        # Windows only; opens a window for ~3 s
```
