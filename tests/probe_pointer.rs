//! Pointer input probes: event names, buttons, double click, capture,
//! hit testing through a drag ghost, and drags that leave the element or
//! the window. All headless, through `Document`'s dispatch methods, which
//! are what the native adapter calls (the native probe shows what the
//! Windows adapter sends).

mod common;

use common::*;
use div::prelude::*;

fn target_scene() -> ViewNode {
    view! {
        <div id="root" style:position="absolute" style:top="0" style:left="0" style:width="800px" style:height="600px">
            <div id="a" style:position="absolute" style:top="100px" style:left="100px" style:width="100px" style:height="100px" style:background-color="rgb(0, 0, 255)"></div>
            <div id="b" style:position="absolute" style:top="100px" style:left="300px" style:width="100px" style:height="100px" style:background-color="rgb(0, 128, 0)"></div>
        </div>
    }
}

#[test]
fn mouse_input_fires_mouse_events_only() {
    let mut ui = app(target_scene);
    let log = Log::new();
    let a = ui.by_id("a");
    log.listen(&a, "a", POINTER_TYPES);
    let (x, y) = center(&a);
    for kind in [
        MouseEventType::Move,
        MouseEventType::Down,
        MouseEventType::Up,
    ] {
        mouse(&mut ui, kind, x, y, MouseButton::Left);
    }
    let seen = log.take();
    report(
        "pointer.mouse-names",
        format!("left click by dispatch_mouse_event: {seen:?}; Chromium sends pointer* before each mouse*"),
    );
    assert_eq!(seen, ["a:mousemove", "a:mousedown", "a:mouseup", "a:click"]);
}

#[test]
fn pointer_input_fires_pointer_events_only() {
    let mut ui = app(target_scene);
    let log = Log::new();
    let a = ui.by_id("a");
    log.listen(&a, "a", POINTER_TYPES);
    let (x, y) = center(&a);
    for kind in [
        PointerEventKind::Move,
        PointerEventKind::Down,
        PointerEventKind::Up,
    ] {
        pointer(&mut ui, 1, kind, x, y);
    }
    let seen = log.take();
    report(
        "pointer.pointer-names",
        format!("tap by dispatch_pointer_event(pointer_names=true): {seen:?}; Chromium follows pointer* with compatibility mouse* events"),
    );
    assert_eq!(
        seen,
        ["a:pointermove", "a:pointerdown", "a:pointerup", "a:click"]
    );
}

#[test]
fn right_button_clicks_and_has_no_contextmenu() {
    let mut ui = app(target_scene);
    let log = Log::new();
    let a = ui.by_id("a");
    log.listen_detail(&a, "a", POINTER_TYPES);
    let (x, y) = center(&a);
    mouse(&mut ui, MouseEventType::Down, x, y, MouseButton::Right);
    mouse(&mut ui, MouseEventType::Up, x, y, MouseButton::Right);
    let seen = log.take();
    report(
        "pointer.right-button",
        format!("right press and release: {seen:?}; Chromium: mousedown, contextmenu, mouseup, auxclick, and no click"),
    );
    assert!(
        seen.iter().any(|e| e.starts_with("a:click")),
        "the right button fires click today"
    );
    assert!(!seen
        .iter()
        .any(|e| e.contains("contextmenu") || e.contains("auxclick")));

    mouse(&mut ui, MouseEventType::Down, x, y, MouseButton::Middle);
    mouse(&mut ui, MouseEventType::Up, x, y, MouseButton::Middle);
    let seen = log.take();
    report(
        "pointer.middle-button",
        format!("middle press and release: {seen:?}; Chromium: auxclick, no click"),
    );
    assert!(seen.iter().any(|e| e.starts_with("a:click")));
}

#[test]
fn double_click_has_no_dblclick() {
    let mut ui = app(target_scene);
    let log = Log::new();
    let a = ui.by_id("a");
    log.listen(&a, "a", &["click", "dblclick"]);
    let (x, y) = center(&a);
    for _ in 0..2 {
        mouse(&mut ui, MouseEventType::Down, x, y, MouseButton::Left);
        mouse(&mut ui, MouseEventType::Up, x, y, MouseButton::Left);
    }
    let seen = log.take();
    report(
        "pointer.dblclick",
        format!("two clicks at one spot: {seen:?}; Event has no click count (`detail`)"),
    );
    assert_eq!(seen, ["a:click", "a:click"]);
}

#[test]
fn click_needs_press_and_release_on_the_same_element() {
    let mut ui = app(target_scene);
    let log = Log::new();
    for id in ["root", "a", "b"] {
        log.listen(&ui.by_id(id), id, &["click"]);
    }
    let (ax, ay) = center(&ui.by_id("a"));
    let (bx, by) = center(&ui.by_id("b"));
    mouse(&mut ui, MouseEventType::Down, ax, ay, MouseButton::Left);
    mouse(&mut ui, MouseEventType::Move, bx, by, MouseButton::Left);
    mouse(&mut ui, MouseEventType::Up, bx, by, MouseButton::Left);
    let seen = log.take();
    report(
        "pointer.click-common-ancestor",
        format!("press on a, release on its sibling b: {seen:?}; Chromium clicks their common ancestor (root, target root), so releasing a drag over a button does not click it"),
    );
    assert_eq!(seen, ["b:click", "root:click"]);
}

#[test]
fn drag_without_capture_loses_the_pointer() {
    let mut ui = app(target_scene);
    let log = Log::new();
    let a = ui.by_id("a");
    log.listen(&a, "a", &["mousemove", "mouseup"]);
    log.listen(&ui.by_id("root"), "root", &["mousemove", "mouseup"]);
    let (x, y) = center(&a);
    mouse(&mut ui, MouseEventType::Down, x, y, MouseButton::Left);
    log.take();
    // Over b, then left of the viewport, then release there.
    mouse(
        &mut ui,
        MouseEventType::Move,
        x + 200.0,
        y,
        MouseButton::Left,
    );
    mouse(&mut ui, MouseEventType::Move, -40.0, y, MouseButton::Left);
    mouse(&mut ui, MouseEventType::Up, -40.0, y, MouseButton::Left);
    let seen = log.take();
    report(
        "pointer.drag-no-capture",
        format!("press on a, move over b, move and release outside the viewport: {seen:?}"),
    );
    // The move over b bubbles to root but never reaches a. Outside the
    // viewport nothing is hit, so neither that move nor the release arrive.
    assert_eq!(seen, ["root:mousemove"]);
}

#[test]
fn pointer_capture_routes_moves_but_is_not_released_on_up() {
    let mut ui = app(target_scene);
    let log = Log::new();
    let a = ui.by_id("a");
    log.listen(
        &a,
        "a",
        &[
            "mousemove",
            "mouseup",
            "click",
            "gotpointercapture",
            "lostpointercapture",
        ],
    );
    let captured = a.clone();
    a.on("mousedown", move |event: &Event| {
        captured.set_pointer_capture(event.pointer_id).unwrap();
    })
    .unwrap();
    let (x, y) = center(&a);
    mouse(&mut ui, MouseEventType::Down, x, y, MouseButton::Left);
    mouse(
        &mut ui,
        MouseEventType::Move,
        x + 200.0,
        y,
        MouseButton::Left,
    );
    mouse(
        &mut ui,
        MouseEventType::Move,
        -40.0,
        -40.0,
        MouseButton::Left,
    );
    mouse(&mut ui, MouseEventType::Up, -40.0, -40.0, MouseButton::Left);
    let during = log.take();
    report(
        "pointer.capture",
        format!("set_pointer_capture(0) in mousedown, drag over b and out of the viewport, release: {during:?}"),
    );
    assert_eq!(
        during,
        ["a:mousemove", "a:mousemove", "a:mouseup", "a:click"]
    );

    // After the release the capture is still held: a hover over b goes to a.
    let (bx, by) = center(&ui.by_id("b"));
    mouse(&mut ui, MouseEventType::Move, bx, by, MouseButton::Left);
    let after = log.take();
    report(
        "pointer.capture-release",
        format!("move over b after the release: {after:?}; Chromium releases capture after pointerup and fires lostpointercapture"),
    );
    assert_eq!(after, ["a:mousemove"]);
}

#[test]
fn capture_on_a_pointer_stream() {
    let mut ui = app(target_scene);
    let log = Log::new();
    let a = ui.by_id("a");
    log.listen(&a, "a", &["pointermove", "pointerup", "click"]);
    let captured = a.clone();
    a.on("pointerdown", move |event: &Event| {
        captured.set_pointer_capture(event.pointer_id).unwrap();
    })
    .unwrap();
    let (x, y) = center(&a);
    pointer(&mut ui, 3, PointerEventKind::Down, x, y);
    pointer(&mut ui, 3, PointerEventKind::Move, x + 200.0, y);
    pointer(&mut ui, 3, PointerEventKind::Up, x + 200.0, y);
    let seen = log.take();
    report(
        "pointer.capture-touch",
        format!("pointer 3 (touch ids are > 0) captured in pointerdown: {seen:?}"),
    );
    assert_eq!(seen, ["a:pointermove", "a:pointerup", "a:click"]);
}

#[test]
fn a_ghost_with_pointer_events_none_is_not_hit() {
    let ui = app(move || {
        view! {
            <div>
                <div id="under" style:position="absolute" style:top="100px" style:left="100px" style:width="200px" style:height="200px" style:background-color="rgb(0, 0, 255)"></div>
                <div id="ghost" style:position="absolute" style:top="150px" style:left="150px" style:width="100px" style:height="100px" style:pointer-events="none" style:opacity="0.5" style:background-color="rgb(255, 0, 0)"></div>
            </div>
        }
    });
    let hit = ui
        .document()
        .hit_test(200.0, 200.0)
        .unwrap()
        .and_then(|e| e.get_attribute("id").unwrap());
    report(
        "pointer.ghost-hit-test",
        format!("hit_test under a pointer-events:none ghost: {hit:?}"),
    );
    assert_eq!(hit.as_deref(), Some("under"));
}

#[test]
fn a_move_carries_one_button_and_no_buttons_mask() {
    // The native adapter sends `PointerButton::Other(0)` with every move,
    // which App maps to `MouseButton::Right` (divui app.rs, `button`). This
    // only shows that the move's button reaches listeners unchanged; the
    // native probe shows the value Windows produces.
    let mut ui = app(target_scene);
    let log = Log::new();
    let a = ui.by_id("a");
    log.listen_detail(&a, "a", &["mousemove"]);
    let (x, y) = center(&a);
    mouse(&mut ui, MouseEventType::Move, x, y, MouseButton::Right);
    let seen = log.take();
    report(
        "pointer.move-button",
        format!("move dispatched with MouseButton::Right: {seen:?}; Event has no `buttons` bitmask or pointer type"),
    );
    assert_eq!(seen, ["a:mousemove(button=2,id=0)"]);
}

fn scroller() -> ViewNode {
    view! {
        <div id="scroller" style:width="200px" style:height="200px" style:overflow="auto">
            <div style:width="1000px" style:height="1000px"></div>
        </div>
    }
}

#[test]
fn ctrl_wheel_can_be_taken_for_zoom() {
    let mut ui = app(scroller);
    let scroller = ui.by_id("scroller");
    let zoom = create_signal(1.0f32);
    scroller
        .on("wheel", move |event: &Event| {
            if Modifiers(event.modifiers as u32).contains(Modifiers::CTRL) {
                event.prevent_default();
                zoom.update(|z| *z *= if event.delta_y > 0.0 { 0.9 } else { 1.1 });
            }
        })
        .unwrap();
    ui.document()
        .dispatch_wheel_event(100.0, 100.0, 0.0, 40.0, Modifiers::CTRL)
        .unwrap();
    ui.run_until_idle();
    let ctrl_top = scroller.scroll_top().unwrap();
    ui.document()
        .dispatch_wheel_event(100.0, 100.0, 0.0, 40.0, Modifiers::NONE)
        .unwrap();
    ui.run_until_idle();
    let plain_top = scroller.scroll_top().unwrap();
    report(
        "input.ctrl-wheel",
        format!("Ctrl+wheel with prevent_default: zoom {}, scroll_top {ctrl_top}; plain wheel: scroll_top {plain_top}. Windows precision touchpads report pinch as Ctrl+wheel; winit's PinchGesture is not forwarded", zoom.get()),
    );
    assert_eq!(ctrl_top, 0.0);
    assert_eq!(plain_top, 40.0);
}

#[test]
fn shift_wheel_scrolls_vertically() {
    let mut ui = app(scroller);
    let scroller = ui.by_id("scroller");
    ui.document()
        .dispatch_wheel_event(100.0, 100.0, 0.0, 40.0, Modifiers::SHIFT)
        .unwrap();
    ui.run_until_idle();
    let (left, top) = (
        scroller.scroll_left().unwrap(),
        scroller.scroll_top().unwrap(),
    );
    report(
        "input.shift-wheel",
        format!("Shift+wheel (delta_y 40): scroll_left {left}, scroll_top {top}; Chromium on Windows and Linux scrolls horizontally"),
    );
    assert_eq!((left, top), (0.0, 40.0));
}

#[test]
fn cancelled_up_suppresses_click() {
    let mut ui = app(|| view! { <div id="b" style:width="100px" style:height="100px"></div> });
    let b = ui.by_id("b");
    let log = Log::new();
    log.listen(&b, "b", &["click"]);
    let (x, y) = center(&b);

    pointer(&mut ui, 3, PointerEventKind::Down, x, y);
    pointer(&mut ui, 3, PointerEventKind::Up, x, y);
    let tap = log.take();
    assert_eq!(tap, vec!["b:click"]);

    // Chromium fires `click` after a cancelled `mouseup` or `pointerup`;
    // only cancelling the `click` itself prevents its activation behavior.
    b.on("mouseup", |event: &Event| event.prevent_default())
        .unwrap();
    b.on("pointerup", |event: &Event| event.prevent_default())
        .unwrap();
    mouse(&mut ui, MouseEventType::Down, x, y, MouseButton::Left);
    mouse(&mut ui, MouseEventType::Up, x, y, MouseButton::Left);
    let cancelled_mouse = log.take();
    assert!(cancelled_mouse.is_empty());
    pointer(&mut ui, 3, PointerEventKind::Down, x, y);
    pointer(&mut ui, 3, PointerEventKind::Up, x, y);
    let cancelled_touch = log.take();
    assert!(cancelled_touch.is_empty());
    report(
        "pointer.click-after-up",
        format!(
            "a tap clicks ({tap:?}); after a cancelled mouseup {cancelled_mouse:?}, after a cancelled pointerup {cancelled_touch:?} (divui/src/document.rs:1108)"
        ),
    );
}
