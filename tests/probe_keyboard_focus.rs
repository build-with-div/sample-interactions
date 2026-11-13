//! Keyboard and focus probes: shortcuts, roving focus, focus traps and
//! restore, focus events, type-ahead and focus scrolling.

mod common;

use common::*;
use div::prelude::*;
use div::testing::Key;

fn id_of(element: Option<Element>) -> Option<String> {
    element.and_then(|e| e.get_attribute("id").unwrap())
}

#[test]
fn a_global_shortcut_sees_key_and_modifiers() {
    let mut ui = app(move || {
        view! {
            <div id="root" tabindex="-1" style:width="800px" style:height="600px">
                <input id="field" type="text" />
            </div>
        }
    });
    let log = Log::new();
    let sink = log.clone();
    ui.document()
        .body()
        .on("keydown", move |event: &Event| {
            let ctrl = Modifiers(event.modifiers as u32).contains(Modifiers::CTRL);
            sink.push(format!("{:?} ctrl={ctrl}", event.key()));
            if ctrl && matches!(event.key(), Some(Key::Char('k' | 'K'))) {
                event.prevent_default();
            }
        })
        .unwrap();
    let field = ui.by_id("field");
    ui.click(&field);
    ui.press_with(Modifiers::CTRL, Key::K);
    ui.press_with(Modifiers::CTRL | Modifiers::SHIFT, Key::P);
    let seen = log.take();
    report(
        "keys.shortcut",
        format!("Ctrl+K and Ctrl+Shift+P from a focused input, seen on <body>: {seen:?}; field value {:?}", field.control_value().unwrap()),
    );
    assert_eq!(seen.len(), 2);
    assert_eq!(field.control_value().unwrap().as_deref(), Some(""));
}

#[test]
fn roving_focus_by_hand() {
    let mut ui = app(move || {
        view! {
            <div>
                <button id="before">"before"</button>
                <div id="list" role="listbox">
                    <div id="o0" tabindex="0">"Apple"</div>
                    <div id="o1" tabindex="-1">"Banana"</div>
                    <div id="o2" tabindex="-1">"Cherry"</div>
                </div>
                <button id="after">"after"</button>
            </div>
        }
    });
    let options: Vec<Element> = ["o0", "o1", "o2"].iter().map(|id| ui.by_id(id)).collect();
    let list = ui.by_id("list");
    let items = options.clone();
    list.on("keydown", move |event: &Event| {
        let Some(current) = items.iter().position(|o| o.has_focus().unwrap()) else {
            return;
        };
        let next = match event.key() {
            Some(Key::ArrowDown) => (current + 1).min(items.len() - 1),
            Some(Key::ArrowUp) => current.saturating_sub(1),
            Some(Key::End) => items.len() - 1,
            Some(Key::Home) => 0,
            _ => return,
        };
        event.prevent_default();
        for (i, item) in items.iter().enumerate() {
            item.set_attribute("tabindex", if i == next { "0" } else { "-1" })
                .unwrap();
        }
        items[next].focus().unwrap();
    })
    .unwrap();

    ui.by_id("before").focus().unwrap();
    ui.press(Key::Tab);
    let entered = id_of(ui.document().focused_element().unwrap());
    ui.press(Key::ArrowDown);
    ui.press(Key::ArrowDown);
    let moved = id_of(ui.document().focused_element().unwrap());
    ui.press(Key::Tab);
    let left = id_of(ui.document().focused_element().unwrap());
    ui.press_with(Modifiers::SHIFT, Key::Tab);
    let back = id_of(ui.document().focused_element().unwrap());
    report(
        "focus.roving",
        format!("Tab into the group -> {entered:?}; ArrowDown x2 -> {moved:?}; Tab -> {left:?}; Shift+Tab -> {back:?}"),
    );
    assert_eq!(entered.as_deref(), Some("o0"));
    assert_eq!(moved.as_deref(), Some("o2"));
    assert_eq!(left.as_deref(), Some("after"));
    assert_eq!(back.as_deref(), Some("o2"));
}

#[test]
fn a_modal_root_traps_and_restores_focus() {
    let mut ui = app(move || {
        view! {
            <div>
                <button id="opener">"open"</button>
                <div id="dialog">
                    <button id="ok">"OK"</button>
                    <button id="cancel">"Cancel"</button>
                </div>
            </div>
        }
    });
    ui.by_id("opener").focus().unwrap();
    let dialog = ui.by_id("dialog");
    ui.document().set_modal_root(Some(&dialog)).unwrap();
    let first = id_of(ui.document().focused_element().unwrap());
    let mut order = Vec::new();
    for _ in 0..3 {
        ui.press(Key::Tab);
        order.push(id_of(ui.document().focused_element().unwrap()));
    }
    // A click outside the modal root.
    let opener = ui.by_id("opener");
    ui.click(&opener);
    let after_outside_click = id_of(ui.document().focused_element().unwrap());
    ui.document().set_modal_root(None).unwrap();
    let restored = id_of(ui.document().focused_element().unwrap());
    report(
        "focus.trap",
        format!("set_modal_root focuses {first:?}; Tab x3 -> {order:?}; click outside -> {after_outside_click:?}; set_modal_root(None) -> {restored:?}"),
    );
    assert_eq!(first.as_deref(), Some("ok"));
    assert!(order
        .iter()
        .all(|id| matches!(id.as_deref(), Some("ok" | "cancel"))));
    assert_eq!(restored.as_deref(), Some("opener"));
}

#[test]
fn focus_events_bubble_with_related_target() {
    let mut ui = app(move || {
        view! {
            <div id="group">
                <button id="x">"x"</button>
                <button id="y">"y"</button>
            </div>
        }
    });
    let log = Log::new();
    for event_type in ["focusin", "focusout", "focus", "blur"] {
        let sink = log.clone();
        ui.by_id("group")
            .on(event_type, move |event: &Event| {
                sink.push(format!(
                    "{}(target={:?},related={:?})",
                    event.event_type,
                    id_of(event.target()),
                    id_of(event.related_target())
                ));
            })
            .unwrap();
    }
    ui.by_id("x").focus().unwrap();
    ui.press(Key::Tab);
    ui.run_until_idle();
    let seen = log.take();
    report(
        "focus.events",
        format!("focus x then Tab to y, listeners on the parent: {seen:?}"),
    );
    assert!(seen
        .iter()
        .any(|e| e.starts_with("focusout(target=Some(\"x\"),related=Some(\"y\")")));
    assert!(seen
        .iter()
        .any(|e| e.starts_with("focusin(target=Some(\"y\")")));
}

#[test]
fn type_ahead_keys_reach_a_focused_listbox() {
    let mut ui = app(move || {
        view! { <div id="list" tabindex="0" role="listbox">"options"</div> }
    });
    let log = Log::new();
    let sink = log.clone();
    ui.by_id("list")
        .on("keydown", move |event: &Event| {
            if let Some(Key::Char(c)) = event.key() {
                sink.push(c.to_string());
            }
        })
        .unwrap();
    ui.by_id("list").focus().unwrap();
    ui.type_text("ch");
    let seen = log.take().concat();
    report(
        "keys.type-ahead",
        format!("typed \"ch\" into a focused tabindex=0 div: {seen:?}; timing between keys is the app's (Event::timestamp_ns)"),
    );
    assert_eq!(seen.to_lowercase(), "ch");
}

#[test]
fn focusing_an_offscreen_item_scrolls_it_into_view() {
    let ui = app(move || {
        let rows: Vec<ViewNode> = (0..50)
            .map(|i| {
                view! { <div id={format!("r{i}")} tabindex="-1" style:height="20px">{format!("row {i}")}</div> }
            })
            .collect();
        let list = view! { <div id="list" style:height="100px" style:overflow-y="auto" style:width="200px"></div> };
        if let ViewNode::Element(e) = &list {
            for row in rows {
                mount_view(e, row);
            }
        }
        list
    });
    let mut ui = ui;
    let list = ui.by_id("list");
    let row = ui.by_id("r30");
    row.focus().unwrap();
    let top = list.scroll_top().unwrap();
    ui.document().update_all().unwrap();
    ui.run_until_idle();
    let top_later = list.scroll_top().unwrap();
    row.scroll_into_view(div::ScrollIntoViewOptions::default())
        .unwrap();
    let top_explicit = list.scroll_top().unwrap();
    report(
        "focus.scroll-into-view",
        format!("focus() on row 30 of a 100px list of 20px rows: scroll_top = {top}, after update_all = {top_later}; explicit scroll_into_view() = {top_explicit}; Chromium's focus() scrolls it into view"),
    );
    assert_eq!(top_later, 0.0);
    assert!(top_explicit > 0.0);
}

#[test]
fn a_div_without_tabindex_does_not_take_focus() {
    let ui = app(move || view! { <div id="plain">"text"</div> });
    let plain = ui.by_id("plain");
    let result = plain.focus();
    let focused = plain.has_focus().unwrap();
    report(
        "focus.non-focusable",
        format!("focus() on a div without tabindex: {result:?}, has_focus = {focused}"),
    );
    assert!(!focused);
}
