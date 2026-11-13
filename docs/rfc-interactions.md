# RFC draft: the interaction platform

A draft for build-with-div/div's `docs/alpha/rfcs/`, in that directory's
format (Problem, Proposal, Affected tracks, Migration), plus a verification
section. Written by the `sample-interactions` dogfood app against Div
`565db46a`. Nothing here is agreed: each part names its owning track, and
the user decides what is scheduled. Feature numbers (F1–F19) and evidence
are in [design.md](design.md).

## Problem

Desktop apps on the web get drag and drop, splitters, overlays, command
palettes, virtual lists, data grids and trees from a few npm packages:

- dnd-kit and react-dnd
- react-resizable-panels
- Radix UI and Floating UI
- cmdk
- TanStack Virtual and Table, and AG Grid
- react-arborist

Those packages are thin. Their power comes from web platform features:
Pointer Events with capture, HTML drag and drop with `DataTransfer`, the
top layer, `<dialog>`, `popover`, anchor positioning, `ResizeObserver`,
`IntersectionObserver`, scroll events, focus, `inert`, ARIA, `position:
sticky`, `user-select`, `touch-action` and `cursor`.

Div renders HTML and CSS as Chromium does, but at `565db46a` most of that
interaction platform is missing or behaves differently. The design's
feature table has the details:

- The native wheel scrolls the wrong way.
- Mouse input never produces `pointer*` events.
- Pointer capture is never released.
- A drag released over a button clicks it.
- There is no HTML drag and drop, `popover`, `<dialog>`, `inert`,
  `ResizeObserver` or `IntersectionObserver`.
- Anchor positioning is a subset.
- The accessibility tree ignores `role` and every ARIA state.

Each app can work around some of this. The probes build a splitter, a
kanban drag and rubber-band selection on today's API. But each workaround
is a private reimplementation of a platform feature, and it diverges from
Chromium in its own way. A Rust port of a package should be able to follow
the original line by line, because the platform under it is the same.

## Proposal

Implement the platform features the packages use, with Chromium 147's
behavior, and expose each through Div's public Rust API with web semantics
underneath. Rust-native shapes are used where they are clearly better:

- typed values instead of strings;
- `Element` handles instead of ids;
- closures instead of `EventListener` objects.

Their names, event order, defaults and edge cases follow the
specification and Chromium, so a port and its WPT tests carry over.

Ports of the packages themselves live in sample repos first, and later in
div-components. They are not part of this RFC.

### F1. Pointer Events and the mouse event model (framework + engine, M)

Chromium's dispatch for every pointer input, native and headless:

- **Event order.** `pointerover`, `pointerenter`, `pointerdown`,
  `pointermove`, `pointerup`, `pointercancel`, `pointerout` and
  `pointerleave`. Then the compatibility mouse events (`mousedown` …) for
  primary pointers, unless `pointerdown` was cancelled. Then `click` on the
  common ancestor of the press and release targets, for the primary button
  only. Other buttons get `auxclick`. A second click within the system's
  double-click time and distance fires `dblclick`, and `detail` counts
  clicks. `contextmenu` follows the secondary button (framework gap 3).
- **Capture.** `setPointerCapture`, `releasePointerCapture` and
  `hasPointerCapture`, with implicit release after `pointerup` or
  `pointercancel`, and `gotpointercapture`/`lostpointercapture` at the next
  pointer event. While captured, the cursor comes from the captor.
- **Event data.** `pointerId` (mouse 1, as Chromium), `pointerType`,
  `isPrimary`, `button` (−1 on moves), `buttons`, `pressure`,
  `tiltX`/`tiltY`, `width`/`height`, `screenX`/`screenY` and
  `offsetX`/`offsetY`.

Rust surface: the existing `Element::set_pointer_capture` and
`release_pointer_capture`, plus `has_pointer_capture`. `Event` gains typed
accessors:

```rust
impl Event {
    pub fn pointer_type(&self) -> Option<PointerType>; // Mouse, Pen, Touch
    pub fn is_primary(&self) -> bool;
    pub fn button(&self) -> Option<MouseButton>;  // None where the DOM says -1
    pub fn buttons(&self) -> Buttons;              // bitflags, DOM values
    pub fn detail(&self) -> u32;
    pub fn pressure(&self) -> f32;
    pub fn tilt(&self) -> (f32, f32);
    pub fn screen_position(&self) -> (f64, f64);
    pub fn offset_position(&self) -> (f32, f32);
}
```

The divui dispatch changes: native mouse input stops taking
`pointer_names = false`. In the engine, `interaction.rs` releases capture
on up and computes the click target. This extends framework gap 4.

### F2. Wheel events (framework, S)

Positive `deltaY` means scrolling down, as in the DOM; today the native
sign is inverted. Lines per notch come from the system setting.
`deltaMode` is set as Chromium sets it. Shift+wheel scrolls horizontally
where Chromium does. On Windows, a precision-touchpad pinch reaches the
page as Ctrl+wheel, as in Chromium.

### F3. HTML drag and drop and `DataTransfer` (engine + framework, L)

- **Drag source.** The `draggable` attribute and the default draggability
  of links, images and selections.
- **Events.** `dragstart`, `drag`, `dragend`, `dragenter`, `dragover`,
  `dragleave` and `drop`, with the specification's drag data store modes.
- **Rust surface.** A `DataTransfer` type: `set_data`, `get_data`,
  `types`, `files` (`PathBuf`s), `items`, `effect_allowed`, `drop_effect`
  and `set_drag_image(&Element, x, y)`. It is reached from drag events
  through `Event::data_transfer()`.
- **Drag image.** Rendered from the element, as Chromium does.
- **OS bridges.** Files and text dragged in target the element under the
  pointer, with position. Dragging out works too. On Windows, Div's own OLE
  `IDropTarget`/`DoDragDrop` replace winit's file-only handler, which
  reports no position. On X11, XDND; on Wayland, `wl_data_device`. The
  same events reach windows opened through `AppSpec`.

Order: in-app first (M), then OS drop in (M), then drag-out (L).

### F4–F7. Top layer, `<dialog>`, `popover`, anchor positioning, `inert` (engine)

- **F4: top layer and `<dialog>`** (engine A3, planned). The work: the
  `<dialog>` element with `show`, `showModal`, `close`, `returnValue`,
  `cancel` on Escape and `::backdrop`, and top-layer rendering and hit
  testing.
- **F5: `popover`.** The attribute's `auto`, `manual` and `hint` states,
  light dismiss, nested auto popovers, `showPopover`, `hidePopover` and
  `togglePopover`, `popovertarget` and `popovertargetaction`, and the
  `beforetoggle`/`toggle` events. It is rendered in the top layer. Size: M.
- **F6: anchor positioning.** Div already has `anchor-name`,
  `position-anchor` and a 9-value `position-area`. The work:
  - `anchor()` and `anchor-size()`;
  - the full `position-area` grammar, including `span-*`;
  - `position-try-fallbacks`, `position-try-order` and `@position-try`
    (typed in `style!`);
  - `position-visibility`;
  - anchors outside the positioned element's containing block and in the
    top layer;
  - the implicit anchor from `popovertarget`.

  Size: M–L.
- **F7: `inert`.** No focus, no hit testing, hidden from accessibility, for
  the subtree. Generalizes today's single modal root, which then becomes
  `showModal`'s inertness. Size: S–M.

With F4–F7, Radix's popover, menu, tooltip and dialog need no placement
code: Floating UI's job becomes CSS, as in Chromium.

### F8–F9. Observers (engine + framework, M each)

```rust
let observer = ResizeObserver::new(|entries: &[ResizeObserverEntry]| { ... });
observer.observe(&element, ResizeObserverBox::BorderBox);
observer.unobserve(&element);

let observer = IntersectionObserver::new(
    IntersectionObserverInit { root: Some(scroller), root_margin: "0px".parse()?, thresholds: vec![0.0, 1.0] },
    |entries: &[IntersectionObserverEntry]| { ... },
);
```

Both follow the specification's delivery points in the frame: resize
observations after layout, with the loop limit and the error behavior, and
intersection after paint-ready geometry. Entries carry
`content_rect`, `border_box_size`, `content_box_size`,
`intersection_ratio` and so on. The window gets a `resize` event.

### F10–F12. Geometry, scrolling, focus (engine, S each)

- Geometry: `elementsFromPoint`, `offsetParent` and `offsetTop`, and
  `visualViewport`.
- Scrolling: `scrollend`, `overscroll-behavior` and scroll anchoring.
- Focus: `focus()` scrolls the element into view, and `preventScroll`
  opts out.

### F13. ARIA in the accessibility tree (engine, M)

Map the `role` attribute and the ARIA states, properties and relations
into AccessKit, as Chromium maps them to platform APIs (core-aam, html-aam).
That covers `selected`, `expanded`, `checked`, `pressed`, `level`,
`posinset`, `setsize`, `rowindex`, `colindex`, `rowcount`, `colcount`,
`multiselectable`, `orientation`, the value range, `activedescendant`,
`controls`, `labelledby`, `describedby` and `live`. The typed setters
(`set_accessibility_role` and friends) stay as the typed path over the
same state.

### F14–F18. CSS and timing (S–M each)

- **F14.** `position: sticky` has nothing missing; it needs pixel coverage
  in grid fixtures.
- **F15.** `user-select` (engine, S–M).
- **F16.** `touch-action`, with touch panning and zooming that cancel
  pointers as Chromium does (engine + framework, M).
- **F17.** The CSS `cursor` keyword set, with the cursor following the
  captor (engine + framework, S).
- **F18.** A per-frame callback before style and layout, as
  `requestAnimationFrame` (framework, S):

  ```rust
  div::request_animation_frame(|time_ms| { ... }) -> FrameHandle;
  ```

### F19. Window chrome (framework, M–L)

- **Drag regions.** Chromium's `app-region: drag | no-drag` CSS property
  marks regions. On Windows, non-client hit testing maps them, so Snap
  layouts work on a custom maximize button. On Linux, they map to winit's
  `drag_window`.
- **Window geometry.** Position, state (normal, minimized, maximized,
  fullscreen), `move` and `resize` events, and the pointer's screen
  position, as typed `WindowSpec` and `WindowHandle` API. Web names are used
  where they exist (`screen_x`, `move_to`, `resize_to`).
- **Not in scope.** Tear-off tabs and docking are app logic on top of F3,
  F1 and window geometry.

## Affected tracks

| Feature | Owner | Size | Notes |
| --- | --- | --- | --- |
| F1 pointer model | framework (divui events, div-platform), engine (`interaction.rs`) | M (S fixes first) | extends framework gap 4; traces that record `click` after a right click or on the release target change |
| F2 wheel | framework (div-platform) | S | first |
| F3 drag and drop | engine (events, defaults, drag image), framework (OS bridges) | L | in-app, then OS in, then out |
| F4 top layer and dialog | engine (A3) | L | planned |
| F5 popover | engine | M | after F4 |
| F6 anchor positioning | engine (style, layout) | M–L | extends what exists |
| F7 inert | engine | S–M | generalizes the modal root |
| F8 ResizeObserver, F9 IntersectionObserver | engine + framework | M each | |
| F10–F12 geometry, scroll, focus details | engine | S each | |
| F13 ARIA mapping | engine (accessibility) | M | div-components' Tabs benefits at once |
| F15 user-select, F16 touch-action | engine (+ framework for touch) | S–M, M | |
| F17 cursor | engine (style), framework (platform) | S | |
| F18 frame callback | framework | S | |
| F19 window chrome | framework (window, div-platform) | M–L | builds on sample-git 20, 21 |
| Testing T1, T2 | framework (TestApp), components (trace harness) | S, S–M | before F1 lands |

div-components consumes all of these when it builds Radix-equivalent
popover, menu, tooltip and select (alpha.2 in the plan), and any later
grid, tree or drag components.

## Migration

Small PRs on `main`, each green:

1. F2 wheel sign, with a unit test on the winit delta mapping. No headless
   trace changes.
2. T1 test helpers, then the S parts of F1 (capture release and its
   events, click target, `auxclick`, `dblclick` and `detail`, `button` on
   moves, `buttons`, `pointerType`). Each PR lists the traces it changes.
3. The F1 unified stream. `mouse*` listeners keep working, and `pointer*`
   listeners start seeing mouse input.
4. F13 and F17, independent of the above.
5. F8, then F4 (A3), F5, F7 and F6.
6. F10–F12, F15, F16 and F18, as they are needed.
7. F3 in-app, F3 OS drop in, F19, and F3 drag-out, per OS.

## Verification

- **Platform features.** Each is measured against Chromium 147 with WPT
  through Div's WPT tooling:
  - `pointerevents`, `uievents`, `html/editing/dnd`;
  - `html/semantics/interactive-elements/the-dialog-element`,
    `html/semantics/popovers`, `inert`;
  - `css/css-anchor-position`, `resize-observer`, `intersection-observer`;
  - `css/css-position/sticky`, `css/css-ui`;
  - `core-aam`/`html-aam` for the accessibility mapping.

  Interaction tests use testdriver Actions, which need T1, plus a mapping
  from Actions to `Document` dispatch.
- **Ported packages.** Each is compared with the original in pinned
  Chromium through behavior traces. Div's
  `tests/fidelity/components` harness is extended (T2) with pointer, drag,
  wheel, touch and file-drop steps, and per-step geometry,
  accessibility-tree and pixel comparison. The design has the fixture list.

## Open questions

- **Pointer ids.** Chromium's mouse `pointerId` is 1. Today Div uses 0
  for the mouse. Change it with F1, or keep it and document the
  difference?
- **`DataTransfer` and files.** Should it expose `PathBuf`s directly (the
  native shape), or `File` objects with paths behind them, as Electron
  does?
- **Window chrome.** Is `app-region` the right name for Div's title-bar
  regions? It is Chromium's own property, used by Electron and Window
  Controls Overlay, but it is not standardized.
