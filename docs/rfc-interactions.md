# RFC draft: interaction primitives

A draft for build-with-div/div's `docs/alpha/rfcs/`, in that directory's
format (Problem, Proposal, Affected tracks, Migration). Written by the
`sample-interactions` dogfood app against Div `565db46a`. Nothing here is
agreed. Each part names its owning track, and the user decides what is
scheduled. Evidence and gap numbers are in [design.md](design.md).

## Problem

Professional desktop apps are built from a small set of interactions:
dragging (handles, splitters, reordering, files), pointer and wheel input,
keyboard models (roving focus, shortcuts, type-ahead), anchored overlays,
and long data views (lists, trees, grids) that a screen reader can read.

At `565db46a`, a Div app can build most of these by hand. A splitter, a
cross-list reorder with a ghost, and rubber-band selection pass headless
with app-owned state, pointer capture and `hit_test`. Three things stop
each app from being correct by default:

1. **The input model differs from the web's** in ways each widget must
   code around. Mouse and touch fire different event families. Capture is
   never released. Right clicks are clicks. A drag released over a button
   clicks it. There is no `dblclick`, no `buttons` and no pointer type. The
   native wheel scrolls the wrong way.
2. **There are no shared behaviors.** Every app re-implements drag
   thresholds, ghosts, auto-scroll, selection semantics, windowing and
   overlay placement, and gets keyboard and accessibility behavior wrong
   in its own way.
3. **Some things cannot be built from app code:** accessible states for
   custom widgets, size change notifications, OS drag and drop, window
   geometry and custom chrome.

## Proposal

Four layers, each usable without the next:

- **A. Input fidelity** (framework with engine): make events behave as
  Chromium's, so web knowledge and Radix-style components carry over.
- **B. Observation and semantics** (engine): size notifications and ARIA
  mapping.
- **C. Behaviors** (div-components `primitives`): headless state machines
  in the style of RFC 0003: drag, resize, selection, windowing, placement.
- **D. Platform** (framework): OS drag and drop, window geometry and
  chrome.

### A. Input fidelity

#### A1. Pointer events as Chromium dispatches them

Every native and headless pointer input produces the `pointer*` event, then
(unless it was cancelled, and for primary pointers) the compatibility
`mouse*` event, as Chromium does. The mouse is pointer id 1 with pointer
type `mouse`. Touch and pen get ids ≥ 2. `Event` gains:

```rust
impl Event {
    pub fn pointer_type(&self) -> Option<PointerType>; // Mouse, Pen, Touch
    pub fn buttons(&self) -> u16;        // DOM bitmask: 1 primary, 2 secondary, 4 auxiliary
    pub fn button(&self) -> Option<MouseButton>; // None on moves (today: Right)
    pub fn detail(&self) -> u32;         // click count for click, dblclick, mousedown
    pub fn screen_position(&self) -> Option<(f32, f32)>; // for tear-off and cross-window drags
    pub fn pressure(&self) -> f32;       // 0.5 while pressed for mouse, as the DOM
    pub fn tilt(&self) -> (f32, f32);
    pub fn is_primary(&self) -> bool;
}
```

Behavior changes:

- **Capture.** An implicit release after `pointerup` or `pointercancel`,
  with `gotpointercapture` and `lostpointercapture` (gap 3). The platform
  cursor comes from the captor while captured.
- **Click.** The target is the common ancestor of the press and release
  targets (gap 5). `click` is for the primary button only. Other buttons
  fire `auxclick` (gap 4). A second click within the platform's
  double-click time and distance fires `dblclick` with `detail` 2 (gap 6).
- **Context menu.** `contextmenu` after a secondary `mousedown` on Windows
  and Linux (and on Shift+F10 or the Menu key), cancellable. This is
  framework gap 3's event, listed here for ordering.
- **Wheel** (P2, gap 1). Fix the sign, so positive `delta_y` means
  "scroll down" as in the DOM. Lines per notch come from the system
  setting, and the line height is Chromium's. Shift+wheel scrolls
  horizontally when the platform does (gap 10).

Native mouse input today reaches `Document::dispatch_pointer_event` with
`pointer_names = false`. The change is in divui's dispatch, not in the
engine's hit testing.

#### A2. Cursor

CSS `cursor` gains `col-resize`, `row-resize`, `ew-resize`, `ns-resize`,
`nesw-resize`, `nwse-resize`, `grab`, `grabbing`, `crosshair`, `copy`,
`alias` and `no-drop`, mapped to winit's cursor icons. The typed style
rejects the rest at compile time, as today. (engine: style; framework:
platform.)

#### A3. Testing helpers

```rust
impl TestApp {
    pub fn pointer(&mut self, id: PointerId) -> PointerDriver<'_>; // .down(at).move_to(at).up()
    pub fn drag(&mut self, from: &Element, to: (f32, f32), steps: u32);
    pub fn right_click(&mut self, element: &Element);
    pub fn double_click(&mut self, element: &Element);
    pub fn wheel(&mut self, at: (f32, f32), delta: (f32, f32), modifiers: Modifiers);
    /// Run one frame: deliver queued scroll and animation events, then lay out.
    pub fn frame(&mut self);
}
```

### B. Observation and semantics

#### B1. Size observation (engine + framework)

```rust
impl Element {
    /// Call `callback` after layout whenever the border box size changes,
    /// and once after the first layout, as a ResizeObserver entry.
    pub fn on_resize(&self, callback: impl Fn(Rect) + 'static) -> Result<ListenerId, Error>;
}
// and a `resize` event on the window's root element when the viewport changes.
```

Delivered at frame time, after layout and before paint, like Chromium's
ResizeObserver. Needed by measured virtual rows (C4) and anchored overlays
(C5).

#### B2. ARIA mapping (engine: accessibility)

Map the web's attributes into the AccessKit tree, so components written
against WAI-ARIA, including the existing Tabs, which already sets
`aria-selected`, are exposed without new Rust API:

- `role` (every role AccessKit has).
- States: `aria-selected`, `aria-expanded`, `aria-checked`,
  `aria-pressed`, `aria-disabled`, `aria-current`.
- Structure: `aria-level`, `aria-posinset`, `aria-setsize`,
  `aria-rowindex`, `aria-colindex`, `aria-rowcount`, `aria-colcount`,
  `aria-multiselectable`, `aria-orientation`.
- Ranges: `aria-valuenow`, `aria-valuemin`, `aria-valuemax`,
  `aria-valuetext`.
- Relations: `aria-activedescendant`, `aria-controls`, `aria-labelledby`,
  `aria-describedby`.

This needs restyle on attribute change (A1 in the alpha plan) only for the
visual side, not for the tree. The typed setters
(`set_accessibility_role` and friends) stay for code that does not use
attributes.

### C. Behaviors (div-components primitives)

All headless: they hold state and attach listeners to elements the caller
renders, like `RovingFocusGroup` today, so a themed component and an app's
own markup can share them. They are shown with RFC 0003's handle-as-prop
style. With context (RFC 0003 request 1), the handles become implicit.

The sketches use syntax `view!` does not have yet: `<For ... let:item>`
children (framework gap 2 is the first step) and props without `Some(..)`
(RFC 0003 request 3). They show intent, not an agreed API.

#### C1. Drag and drop: `DragDrop<T>`

```rust
let dnd = DragDrop::<CardId>::new(DragOptions {
    threshold: 4.0,                  // px before a press becomes a drag
    auto_scroll: AutoScroll::nearest_scroller(48.0), // edge band in px
    keyboard: true,                  // Space lifts, arrows move, Space drops, Escape cancels
    ..Default::default()
});

view! {
    <div>
        <For each=move || cards.get() key=|c| c.id let:card>
            <Draggable dnd={dnd.clone()} item={card.id}
                preview=Preview::Clone                 // or Preview::View(|id| view! { ... })
                on_drag_start=move |id| set_dragging(Some(id))>
                <Card card=card />
            </Draggable>
        </For>
        <DropTarget dnd={dnd.clone()}
            accepts=move |id: &CardId| can_move(*id, column)
            on_drag_over=move |over: DragOver<CardId>| over.insertion_index()   // drives an indicator
            on_drop=move |drop: Drop<CardId>| move_card(drop.item, column, drop.index) />
    </div>
}
```

- Built on A1: pointer capture on the draggable, `hit_test` through the
  preview (`pointer-events: none`), and drop targets found by a parent walk.
- The preview goes on the top layer (A3 in the alpha plan). Until then it
  is a fixed portal at the end of `<body>`, which the probes show works.
- Auto-scroll needs a per-frame tick. It uses `task::interval` until the
  framework has a frame callback.
- Keyboard drag and live-region announcements (`AccessibilityLive` exists)
  follow dnd-kit's model.
- The OS bridge (D1) feeds external files into the same `DropTarget`
  (`Drop::Files(Vec<PathBuf>)`), and a drag that leaves the window can
  become an OS drag.
- `Sortable` (a list of keyed items with insertion indicators) is a thin
  layer on top, also used for column reorder and tabs.

#### C2. Resizable panels

```rust
view! {
    <PanelGroup direction=Direction::Horizontal storage=Storage::key("main-layout")>
        <Panel min=10.0 default=25.0 collapsible=true>{sidebar}</Panel>
        <PanelHandle />                                   // role=separator, aria-valuenow, col-resize cursor
        <Panel min=30.0>
            <PanelGroup direction=Direction::Vertical>
                <Panel default=70.0>{editor}</Panel>
                <PanelHandle />
                <Panel min=10.0>{terminal}</Panel>
            </PanelGroup>
        </Panel>
    </PanelGroup>
}
```

Sizes are percentages of the group, constrained by min, max and collapse.
Keyboard behavior follows the WAI-ARIA window splitter pattern: arrows
step, Home and End go to the limits, and Enter toggles collapse. The
layout uses `flex-basis` on one flex level per group, so it does not nest
flex columns more than the app does. Sizes are saved through a
`Storage` hook (the app data dir is sample-git gap 20).

#### C3. Selection model

```rust
let selection = SelectionModel::<RowId>::new(SelectionMode::Multiple);
// in a row's mousedown / keydown:
selection.handle_pointer(row.id, Held::of(event), &order);   // click, Ctrl, Shift, Ctrl+Shift
selection.handle_key(Key::ArrowDown, Held::of(event), &order); // Shift extends from the anchor
selection.select_all(&order);
selection.is_selected(id) // reactive
```

`order` is the visible order: a slice of keys, or a closure from index to
key for virtual lists. The anchor and focus follow Windows Explorer and
macOS Finder behavior, as Chromium's listbox does. Shared by listbox, tree,
grid and rubber band.

#### C4. Virtual list

```rust
let list = VirtualListHandle::new();
view! {
    <VirtualList handle={list.clone()}
        each=move || rows.get()           // Memo<Vec<Row>> or a count + index closure
        key=|row| row.id
        estimate=24.0                     // px; measured rows replace it (B1)
        overscan=6
        let:row>
        <RowView row=row />
    </VirtualList>
}
list.scroll_to_index(1_000, ScrollAlign::Center);
```

- A spacer, absolutely positioned rows, and the scroll event at frame
  time: the shape the probe uses, which mounts 100,000 rows in 51–118 ms.
- Variable heights: each row reports its size through B1. A Fenwick tree
  of heights maps scroll offset to index.
- Keyed reuse keeps focus and inline edits when a row scrolls out and back.
  Focus on an unmounted row is kept by key and restored when it is mounted.
- Accessibility: `aria-setsize`, `aria-posinset` and `aria-rowindex` (B2),
  so screen readers see the whole list.
- Depends on the framework making a `For` swap cheaper (gap 17: 0.9–1.4 ms
  per row swapped in the test profile), or on the list keeping a
  pool of row views.

#### C5. Anchored overlays

```rust
let open = create_signal(false);
view! {
    <PopoverAnchor id="trigger">
        <Button on:click=move |_| open.update(|o| *o = !*o)>"Options"</Button>
    </PopoverAnchor>
    <Popover open=open anchor="trigger"
        placement=Placement { side: Side::Bottom, align: Align::Start, offset: 4.0,
                              flip: true, shift: true, match_width: false }
        dismiss=Dismiss::default()>       // Escape, outside press, focus out (DismissableLayer)
        <MenuContent>...</MenuContent>
    </Popover>
}
```

- Placement is a pure function, `place(anchor: Rect, floating: Size,
  viewport: Rect, placement) -> Point`, that is testable without a
  document. It reruns on scroll (frame time), on B1 resize of the anchor or
  the content, and on viewport resize.
- Rendered in the top layer (A3 in the alpha plan). Until then, a fixed
  portal (works today).
- `Tooltip` (hover and focus with delays from `task::sleep`, aria
  `describedby`), `DropdownMenu` and `ContextMenu` (on A1's `contextmenu`;
  a native popup menu is framework gap 3's alternative), and `Toaster` (a
  top-layer region with `AccessibilityLive::Polite`) are compositions,
  matching RFC 0003's alpha.2 list.

#### C6. Tree view and data grid

Compositions of the above. Tree view is M, data grid is L.

```rust
view! {
    <TreeView selection={selection.clone()} load=move |node: &Path| async move { list_dir(node).await }
        root=PathBuf::from(".") key=|p| p.clone() let:node>
        <TreeRow node=node />            // role=treeitem, aria-level, aria-expanded (B2)
    </TreeView>

    <DataGrid rows=rows key=|r| r.id selection={selection.clone()}
        sort=sort on_sort=move |s| sort.set(s)>
        <Column id="name" header="Name" width=240.0 min=80.0 resizable=true reorderable=true let:row>
            {row.name.clone()}
        </Column>
        <Column id="size" header="Size" width=100.0 align=Align::End let:row>
            {format_size(row.size)}
        </Column>
    </DataGrid>
}
```

- The grid follows the WAI-ARIA grid keyboard model: arrows, Home and End,
  Ctrl+Home and Ctrl+End, PageUp and PageDown, F2 or Enter to edit, and
  Escape to cancel.
- Its parts: a sticky header (works today), column resize through C2's
  handle logic, column reorder through C1's `Sortable`, rows through C4
  in two dimensions, and selection through C3.
- Gap 18 (60–128 ms to relay out 320 cells after one width change) has to
  be understood before the grid targets thousands of visible cells.

### D. Platform

#### D1. OS drag and drop (framework: div-platform per OS)

```rust
// Files and text dragged in, targeted at elements, with feedback:
element.on("filedragover", |e| { e.set_drop_effect(DropEffect::Copy); });
element.on("filedrop", |e| { let paths = e.files(); /* ... */ });

// Dragging out of the app (from a Draggable or by hand):
div::dnd::start_os_drag(DragData::files(vec![path]).text("notes.md"), DropEffects::COPY | DropEffects::MOVE)
    .await; // -> Option<DropEffect>
```

- **Windows.** An `IDropTarget` of Div's own replaces winit's file-only
  handler. winit reports no position for a hovered file, and Div needs one
  to choose the element under the pointer. Drag-out uses `DoDragDrop`.
- **Linux.** XDND on X11, and `wl_data_device` on Wayland.
- **Sizes.** Drop in: M. Drag out: L.
- **Today's events.** `App::on_platform_event` stays as the low-level
  hook, and the same events reach `AppSpec` windows.

#### D2. Window geometry and chrome (framework: window, div-platform)

```rust
WindowSpec::new("doc-2")
    .position(Position::Screen(x, y))        // or ::Centered, ::Cascade
    .state(WindowState::Maximized)
    .title_bar(TitleBar::Custom);            // frameless, keeps OS resize borders and Snap

impl WindowHandle {
    pub fn outer_rect(&self) -> Option<ScreenRect>;
    pub fn set_outer_rect(&self, rect: ScreenRect);
    pub fn state(&self) -> WindowState;   // Normal, Minimized, Maximized, Fullscreen
    pub fn set_state(&self, state: WindowState);
    pub fn start_drag_move(&self);           // winit drag_window, from a pointerdown
    pub fn start_drag_resize(&self, edge: ResizeEdge);
}
// Events on the window root: resize, move, statechange, activate/deactivate (sample-git 21).
// Custom title bar regions, so Windows shows Snap layouts on the maximize button:
view! { <div data-window-region="caption">...<button data-window-region="maximize">...</button></div> }
div::window::cursor_screen_position() -> Option<(f64, f64)>;
```

Tear-off tabs and docking are app code on top of D2 and C1: a tab dragged
outside its window opens `window::open` at the screen pointer position, and
the drag continues there. They are not a primitive.

## Affected tracks

| Part | Owner | Size | Consumers | Notes |
| --- | --- | --- | --- | --- |
| A1 pointer parity | framework (divui events), engine (capture and click in `interaction.rs`) | M | every interactive component | extends framework gap 4; the click target and capture changes alter today's traces |
| A1 wheel | framework (div-platform) | S | every scroller | sign fix first, on its own |
| A2 cursors | engine (style), framework (platform) | S | C1, C2 | |
| A3 test helpers | framework (testing) | S | all tracks' tests | |
| B1 size observation | engine (layout), framework (delivery) | M | C4, C5 | |
| B2 ARIA mapping | engine (accessibility) | M | components (Tabs today), C2–C6 | |
| C1–C6 behaviors | components | S–L each | apps | after A1; C5 after the top layer (A3 in the alpha plan) |
| D1 OS DnD | framework (div-platform) | M + L | C1, apps | per OS |
| D2 window chrome | framework (window, div-platform) | M–L | apps | builds on sample-git 20 and 21 |

## Migration

Small PRs on `main`, each green:

1. The wheel sign fix (S), with a native-adapter unit test on winit
   deltas. It changes no headless trace.
2. Capture release, click target, `auxclick`, `dblclick` and the move
   button (S each, or one series under framework gap 4), with the test
   helpers (A3) in the first PR that needs them. Traces that recorded
   `click` on the release target or after a right click change. The PRs
   say so.
3. A1's unified `pointer*` + `mouse*` stream (M). Listeners on `mouse*`
   keep working. Listeners on `pointer*` start seeing mouse input.
4. Cursors (S) and B2 ARIA mapping (M), independent of 1–3.
5. B1 size observation (M), then C3, C2, C4 in div-components.
6. The top layer (alpha plan A3), then C5, then C1 and C6.
7. D1 file drop in, D2 window chrome, and D1 drag-out, per OS.

## Open questions

- **Pointer ids.** Use Chromium's (mouse 1, others from 2), or keep 0 for
  the mouse and document the difference?
- **Behavior placement.** Should the C behaviors live in
  `div-components::primitives` (headless, as RFC 0003) or in a separate
  crate, so apps that bring their own design system don't pull in the
  shadcn theme?
- **Long lists.** Fix `For`'s per-row cost (gap 17) in the framework, or
  have `VirtualList` keep a pool of row views? The first helps every app.
  The second only helps lists.
