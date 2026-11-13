//! Overlay probes: what an app can build today without a top layer or
//! anchor positioning. Popovers escaping a clipping container, stacking,
//! outside-press dismissal, and the events an anchored overlay needs to
//! follow its anchor.

mod common;

use common::*;
use div::prelude::*;

const BLUE: [u8; 4] = [0, 0, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];

fn clipped_anchor_scene() -> ViewNode {
    view! {
        <div>
            <div id="clip" style:position="absolute" style:top="50px" style:left="50px" style:width="100px" style:height="100px" style:overflow="hidden" style:background-color="rgb(0, 0, 255)">
                <div id="anchor" style:position="relative" style:width="40px" style:height="40px" style:background-color="rgb(0, 128, 0)">
                    <div id="inline-popover" style:position="absolute" style:top="200px" style:left="0" style:width="80px" style:height="40px" style:background-color="rgb(255, 0, 0)"></div>
                </div>
            </div>
            <div id="raised" style:position="absolute" style:top="0" style:left="400px" style:width="200px" style:height="200px" style:z-index="10" style:background-color="rgb(0, 0, 255)"></div>
        </div>
    }
}

#[test]
fn a_popover_inside_a_clip_is_clipped_and_a_body_level_one_is_not() {
    let ui = app(clipped_anchor_scene);
    // The inline popover sits at y = 50 + 200; the clip hides it.
    let inline = pixel(&ui, 60, 260);
    // Portal by hand: a fixed box appended to <body>, placed from the
    // anchor's bounding_rect, with a z-index above the page.
    let anchor = ui.by_id("anchor").bounding_rect().unwrap().unwrap();
    let document = ui.document().clone();
    let portal = with_document(
        &document,
        || view! { <div id="portal" style:position="fixed" style:width="80px" style:height="40px" style:z-index="100" style:background-color="rgb(255, 0, 0)"></div> },
    );
    mount_view(&ui.document().body(), portal);
    let portal = ui.by_id("portal");
    portal
        .set_left(div::typed_style::Length::px(anchor.x))
        .unwrap();
    portal
        .set_top(div::typed_style::Length::px(anchor.y + anchor.height))
        .unwrap();
    let ported = pixel(
        &ui,
        anchor.x as u32 + 10,
        (anchor.y + anchor.height) as u32 + 10,
    );
    report(
        "overlay.portal",
        format!("popover inside overflow:hidden at its offset: {inline:?}; body-level position:fixed copy under the anchor: {ported:?}"),
    );
    assert_ne!(inline, RED);
    assert_eq!(ported, RED);
}

#[test]
fn a_fixed_overlay_stacks_above_a_z_indexed_sibling() {
    let ui = app(clipped_anchor_scene);
    let document = ui.document().clone();
    let overlay = with_document(
        &document,
        || view! { <div style:position="fixed" style:top="100px" style:left="450px" style:width="50px" style:height="50px" style:z-index="100" style:background-color="rgb(255, 0, 0)"></div> },
    );
    mount_view(&ui.document().body(), overlay);
    let over = pixel(&ui, 470, 120);
    let low = with_document(
        &document,
        || view! { <div style:position="fixed" style:top="100px" style:left="550px" style:width="20px" style:height="20px" style:z-index="5" style:background-color="rgb(255, 0, 0)"></div> },
    );
    mount_view(&ui.document().body(), low);
    let under = pixel(&ui, 555, 105);
    report(
        "overlay.z-order",
        format!(
            "z-index 100 overlay over a z-index 10 box: {over:?}; z-index 5 overlay: {under:?}"
        ),
    );
    assert_eq!(over, RED);
    assert_eq!(under, BLUE);
}

#[test]
fn outside_press_dismissal_from_a_capture_listener() {
    let mut ui = app(clipped_anchor_scene);
    let log = Log::new();
    let popover = ui.by_id("anchor");
    let sink = log.clone();
    let inside = popover.clone();
    ui.document()
        .body()
        .on_capture("mousedown", move |event: &Event| {
            let target = event.target().unwrap();
            sink.push(if inside.contains(&target).unwrap() {
                "inside"
            } else {
                "outside: dismiss"
            });
        })
        .unwrap();
    let raised = ui.by_id("raised");
    ui.click(&popover);
    ui.click(&raised);
    let seen = log.take();
    report(
        "overlay.dismiss",
        format!("capture-phase mousedown on <body> + Element::contains: {seen:?}"),
    );
    assert_eq!(seen, ["inside", "outside: dismiss"]);
}

#[test]
fn an_anchor_can_follow_scrolling_but_not_resizing() {
    let mut ui = app(move || {
        let rows: Vec<ViewNode> = (0..20)
            .map(|i| view! { <div style:height="20px">{format!("row {i}")}</div> })
            .collect();
        let list = view! { <div id="list" style:height="100px" style:overflow-y="auto" style:width="200px"></div> };
        if let ViewNode::Element(e) = &list {
            for row in rows {
                mount_view(e, row);
            }
        }
        list
    });
    let log = Log::new();
    log.listen(&ui.by_id("list"), "list", &["scroll"]);
    log.listen(&ui.document().body(), "body", &["scroll", "resize"]);
    ui.by_id("list").scroll_to(0.0, 60.0).unwrap();
    ui.run_until_idle();
    let before_frame = log.snapshot();
    // Scroll events are queued and delivered at the next frame, as in
    // Chromium; TestApp::run_until_idle does not deliver them.
    ui.document().dispatch_pending_events().unwrap();
    let after_frame = log.take();
    ui.document()
        .dispatch_wheel_event(50.0, 50.0, 0.0, 40.0, Modifiers::NONE)
        .unwrap();
    ui.document().dispatch_pending_events().unwrap();
    ui.run_until_idle();
    ui.document()
        .set_viewport(ViewportMetrics::from_logical_size(900.0, 700.0, 1.0).unwrap())
        .unwrap();
    ui.document().update_all().unwrap();
    ui.run_until_idle();
    let seen = log.take();
    report(
        "overlay.reposition-events",
        format!("scroll_to: {before_frame:?} until dispatch_pending_events, then {after_frame:?}; a wheel, then a viewport resize: {seen:?}; there is no resize event or ResizeObserver to reposition on"),
    );
    assert!(before_frame.is_empty());
    assert_eq!(after_frame, ["list:scroll"]);
    assert!(seen.iter().any(|e| e == "list:scroll"));
    assert!(!seen.iter().any(|e| e.ends_with(":resize")));
}
