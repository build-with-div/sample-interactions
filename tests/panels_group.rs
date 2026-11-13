//! The react-resizable-panels port on Div: the DOM it renders, pointer
//! drags (mouse and touch), separator keys and nested groups, driven
//! through `Document` dispatch as the native adapter drives it.

mod common;

use common::*;
use div::prelude::*;
use sample_interactions::panels::layout::{adjust_layout_by_delta, Trigger};
use sample_interactions::panels::{
    document_cursor, Group, GroupHandle, Orientation, Panel, Separator,
};
use std::cell::RefCell;
use std::rc::Rc;

const SEPARATOR_WIDTH: f32 = 4.0;

fn host() -> div::testing::TestApp {
    app(|| {
        view! {
            <div id="host" style:width="800px" style:height="400px"></div>
        }
    })
}

/// A separator for a horizontal group: 4px wide, stretched to full height.
fn separator(id: &str) -> Separator {
    Separator::new(id).style(|element| element.set_width(LengthValue::px(SEPARATOR_WIDTH)).unwrap())
}

/// A separator for a vertical group: 4px high, stretched to full width.
fn row_separator(id: &str) -> Separator {
    Separator::new(id).style(|element| {
        element
            .set_height(LengthValue::px(SEPARATOR_WIDTH))
            .unwrap()
    })
}

/// Three panels across 800px with two 4px separators: 792px of panels.
fn three_panels(
    ui: &div::testing::TestApp,
    changes: Rc<RefCell<Vec<Vec<f64>>>>,
    committed: Rc<RefCell<Vec<Vec<f64>>>>,
) -> GroupHandle {
    Group::new("group", Orientation::Horizontal)
        .panel(Panel::new("left").default_size("25%").min_size("10%"))
        .separator(separator("sep-1"))
        .panel(Panel::new("center"))
        .separator(separator("sep-2"))
        .panel(
            Panel::new("right")
                .default_size("25%")
                .min_size("20%")
                .collapsible(true),
        )
        .on_layout_change(move |layout| changes.borrow_mut().push(layout.to_vec()))
        .on_layout_changed(move |layout| committed.borrow_mut().push(layout.to_vec()))
        .mount(ui.document(), &ui.by_id("host"))
        .expect("group mounts")
}

fn attribute(element: &Element, name: &str) -> Option<String> {
    element.get_attribute(name).unwrap()
}

fn width(element: &Element) -> f32 {
    element.bounding_rect().unwrap().unwrap().width
}

#[test]
fn renders_the_package_dom() {
    let ui = host();
    let group = three_panels(&ui, Rc::default(), Rc::default());
    assert_eq!(group.layout(), vec![25.0, 50.0, 25.0]);

    let separator = ui.by_id("sep-1");
    assert_eq!(attribute(&separator, "role").as_deref(), Some("separator"));
    assert_eq!(
        attribute(&separator, "aria-orientation").as_deref(),
        Some("vertical")
    );
    assert_eq!(
        attribute(&separator, "aria-controls").as_deref(),
        Some("left")
    );
    assert_eq!(
        attribute(&separator, "aria-valuenow").as_deref(),
        Some("25")
    );
    assert_eq!(
        attribute(&separator, "aria-valuemin").as_deref(),
        Some("10")
    );
    // The right panel can collapse, so the left one can reach 100%.
    assert_eq!(
        attribute(&separator, "aria-valuemax").as_deref(),
        Some("100")
    );
    assert_eq!(attribute(&separator, "tabindex").as_deref(), Some("0"));
    assert_eq!(
        attribute(&separator, "data-separator").as_deref(),
        Some("inactive")
    );
    assert_eq!(
        attribute(&ui.by_id("left"), "data-panel").as_deref(),
        Some("true")
    );
    assert_eq!(
        attribute(group.element(), "data-group").as_deref(),
        Some("true")
    );

    // Panels grow by their percentages: 25%, 50% and 25% of 792px.
    let widths: Vec<f32> = ["left", "center", "right"]
        .iter()
        .map(|id| width(&ui.by_id(id)))
        .collect();
    report(
        "panels.dom",
        format!("layout {:?}, panel widths {widths:?}", group.layout()),
    );
    assert_eq!(widths, vec![198.0, 396.0, 198.0]);
}

#[test]
fn mouse_drag_resizes_and_commits_once() {
    let mut ui = host();
    let changes = Rc::new(RefCell::new(Vec::new()));
    let committed = Rc::new(RefCell::new(Vec::new()));
    let group = three_panels(&ui, changes.clone(), committed.clone());
    let separator = ui.by_id("sep-1");
    let clicks = Log::new();
    clicks.listen(&separator, "sep", &["click"]);
    let (x, y) = center(&separator);

    mouse(&mut ui, MouseEventType::Move, x, y, MouseButton::Left);
    assert_eq!(
        attribute(&separator, "data-separator").as_deref(),
        Some("hover")
    );
    assert_eq!(document_cursor(ui.document()), Some("ew-resize"));

    mouse(&mut ui, MouseEventType::Down, x, y, MouseButton::Left);
    assert!(
        separator.has_focus().unwrap(),
        "pointerdown focuses the separator"
    );
    assert_eq!(
        attribute(&separator, "data-separator").as_deref(),
        Some("active")
    );
    // 99px of 792px is 12.5%.
    for step in 1..=3 {
        mouse(
            &mut ui,
            MouseEventType::Move,
            x + 33.0 * step as f32,
            y,
            MouseButton::Left,
        );
    }
    assert_eq!(group.layout(), vec![37.5, 37.5, 25.0]);
    // Both callbacks also ran once at mount, as in the package.
    assert_eq!(
        changes.borrow().len(),
        4,
        "onLayoutChange at mount and on each move"
    );
    assert_eq!(
        *committed.borrow(),
        vec![vec![25.0, 50.0, 25.0]],
        "onLayoutChanged waits for the release"
    );

    mouse(&mut ui, MouseEventType::Up, x + 99.0, y, MouseButton::Left);
    assert_eq!(group.layout(), vec![37.5, 37.5, 25.0]);
    assert_eq!(
        *committed.borrow(),
        vec![vec![25.0, 50.0, 25.0], vec![37.5, 37.5, 25.0]]
    );
    assert!(!separator.has_focus().unwrap(), "a drag releases focus");
    assert_eq!(
        attribute(&separator, "data-separator").as_deref(),
        Some("inactive")
    );
    assert_eq!(
        attribute(&separator, "aria-valuenow").as_deref(),
        Some("37.5")
    );
    assert!(
        clicks.take().is_empty(),
        "the click after a drag is suppressed"
    );
    assert_eq!(document_cursor(ui.document()), None);
    assert_eq!(width(&ui.by_id("left")), 297.0);

    // A later click is not suppressed.
    let (x, y) = center(&separator);
    mouse(&mut ui, MouseEventType::Down, x, y, MouseButton::Left);
    mouse(&mut ui, MouseEventType::Up, x, y, MouseButton::Left);
    assert_eq!(clicks.take(), vec!["sep:click"]);
    report(
        "panels.mouse-drag",
        "a 99px drag gives [37.5, 37.5, 25]; 3 layout changes and 1 commit after the mount notification; focus released; the click after the drag suppressed",
    );
}

#[test]
fn drag_past_the_minimum_stops_and_flags_the_cursor() {
    let mut ui = host();
    let group = three_panels(&ui, Rc::default(), Rc::default());
    let (x, y) = center(&ui.by_id("sep-1"));
    mouse(&mut ui, MouseEventType::Down, x, y, MouseButton::Left);
    mouse(
        &mut ui,
        MouseEventType::Move,
        x - 150.0,
        y,
        MouseButton::Left,
    );
    assert_eq!(group.layout(), vec![10.0, 65.0, 25.0]);
    mouse(
        &mut ui,
        MouseEventType::Move,
        x - 160.0,
        y,
        MouseButton::Left,
    );
    assert_eq!(group.layout(), vec![10.0, 65.0, 25.0]);
    assert_eq!(document_cursor(ui.document()), Some("e-resize"));
    mouse(&mut ui, MouseEventType::Up, x - 160.0, y, MouseButton::Left);
    report(
        "panels.min-cursor",
        "left stops at its 10% minimum; the package's cursor is e-resize (not applied: F17)",
    );
}

#[test]
fn a_collapsible_panel_collapses_past_half_its_minimum() {
    let mut ui = host();
    let group = three_panels(&ui, Rc::default(), Rc::default());
    let (x, y) = center(&ui.by_id("sep-2"));
    mouse(&mut ui, MouseEventType::Down, x, y, MouseButton::Left);
    // Right is 25% with a 20% minimum: dragging 9% of 792px keeps it at the
    // minimum; 16% (past half the minimum below it) collapses it.
    mouse(
        &mut ui,
        MouseEventType::Move,
        x + 0.09 * 792.0,
        y,
        MouseButton::Left,
    );
    assert_eq!(group.layout(), vec![25.0, 55.0, 20.0]);
    mouse(
        &mut ui,
        MouseEventType::Move,
        x + 0.16 * 792.0,
        y,
        MouseButton::Left,
    );
    assert_eq!(group.layout(), vec![25.0, 75.0, 0.0]);
    mouse(
        &mut ui,
        MouseEventType::Up,
        x + 0.16 * 792.0,
        y,
        MouseButton::Left,
    );
    report(
        "panels.collapse",
        "right collapses to 0 past 10% below its 20% minimum",
    );
}

#[test]
fn separator_keys_follow_the_package() {
    let mut ui = host();
    let group = three_panels(&ui, Rc::default(), Rc::default());
    let separator = ui.by_id("sep-1");
    separator.focus().unwrap();
    assert_eq!(
        attribute(&separator, "data-separator").as_deref(),
        Some("focus")
    );

    ui.press(Key::ArrowRight);
    assert_eq!(group.layout(), vec![30.0, 45.0, 25.0]);
    ui.press(Key::ArrowDown);
    assert_eq!(
        group.layout(),
        vec![30.0, 45.0, 25.0],
        "a horizontal group ignores up and down"
    );
    ui.press(Key::Home);
    assert_eq!(group.layout(), vec![10.0, 65.0, 25.0]);

    let constraints = group.constraints();
    let before = group.layout();
    ui.press(Key::End);
    let expected = adjust_layout_by_delta(
        100.0,
        &before,
        &constraints,
        [0, 1],
        &before,
        Trigger::Keyboard,
    )
    .unwrap();
    assert_eq!(group.layout(), expected);

    // F6 moves focus to the next separator, Shift+F6 back.
    ui.press(Key::Function(6));
    assert!(ui.by_id("sep-2").has_focus().unwrap());
    ui.press_with(Modifiers::SHIFT, Key::Function(6));
    assert!(separator.has_focus().unwrap());
    report(
        "panels.keys",
        format!(
            "ArrowRight +5, Home to the minimum, End {:?}, F6 cycles",
            group.layout()
        ),
    );
}

#[test]
fn enter_collapses_and_restores() {
    let mut ui = host();
    let group = Group::new("vertical", Orientation::Vertical)
        .panel(
            Panel::new("top")
                .default_size("30%")
                .min_size("20%")
                .collapsible(true),
        )
        .separator(row_separator("sep"))
        .panel(Panel::new("bottom"))
        .mount(ui.document(), &ui.by_id("host"))
        .unwrap();
    let separator = ui.by_id("sep");
    assert_eq!(
        attribute(&separator, "aria-orientation").as_deref(),
        Some("horizontal")
    );
    separator.focus().unwrap();
    ui.press(Key::ArrowDown);
    assert_eq!(group.layout(), vec![35.0, 65.0]);
    ui.press(Key::Enter);
    assert_eq!(group.layout(), vec![0.0, 100.0]);
    ui.press(Key::Enter);
    assert_eq!(
        group.layout(),
        vec![35.0, 65.0],
        "Enter restores the size before the collapse"
    );
    report("panels.enter", "Enter collapses top to 0 and restores 35%");
}

#[test]
fn touch_drags_with_pointer_events() {
    let mut ui = host();
    let group = three_panels(&ui, Rc::default(), Rc::default());
    let (x, y) = center(&ui.by_id("sep-1"));
    pointer(&mut ui, 2, PointerEventKind::Down, x, y);
    pointer(&mut ui, 2, PointerEventKind::Move, x + 99.0, y);
    pointer(&mut ui, 2, PointerEventKind::Up, x + 99.0, y);
    assert_eq!(group.layout(), vec![37.5, 37.5, 25.0]);
    report(
        "panels.touch",
        "a touch drag (pointer id 2, pointer* events) gives [37.5, 37.5, 25]",
    );
}

#[test]
fn pixel_constraints_convert_to_percent() {
    let ui = host();
    let group = Group::new("px", Orientation::Horizontal)
        .panel(Panel::new("a").default_size(198.0).min_size(99.0))
        .separator(separator("sep"))
        .panel(Panel::new("b"))
        .mount(ui.document(), &ui.by_id("host"))
        .unwrap();
    // 796px of panels: 198px is 24.874%, 99px is 12.437%.
    let constraints = group.constraints();
    assert_eq!(constraints[0].default_size, Some(24.874));
    assert_eq!(constraints[0].min_size, 12.437);
    assert_eq!(group.layout(), vec![24.874, 75.126]);
}

#[test]
fn nested_groups_resize_independently() {
    let mut ui = host();
    let outer = Group::new("outer", Orientation::Horizontal)
        .panel(Panel::new("side").default_size("25%"))
        .separator(separator("outer-sep"))
        .panel(Panel::new("main"))
        .mount(ui.document(), &ui.by_id("host"))
        .unwrap();
    let inner = Group::new("inner", Orientation::Vertical)
        .panel(Panel::new("editor"))
        .separator(row_separator("inner-sep"))
        .panel(Panel::new("terminal"))
        .mount(ui.document(), outer.panel_content("main").unwrap())
        .unwrap();
    assert_eq!(inner.layout(), vec![50.0, 50.0]);

    // 396px of the 400px-high inner group: 39.6px is 10%.
    let (x, y) = center(&ui.by_id("inner-sep"));
    mouse(&mut ui, MouseEventType::Down, x, y, MouseButton::Left);
    mouse(
        &mut ui,
        MouseEventType::Move,
        x,
        y + 39.6,
        MouseButton::Left,
    );
    mouse(&mut ui, MouseEventType::Up, x, y + 39.6, MouseButton::Left);
    assert_eq!(inner.layout(), vec![60.0, 40.0]);
    assert_eq!(outer.layout(), vec![25.0, 75.0]);
    report(
        "panels.nested",
        "dragging the inner vertical separator moves only the inner group",
    );
}

#[test]
fn a_disabled_separator_is_inert() {
    let mut ui = host();
    let group = Group::new("disabled", Orientation::Horizontal)
        .panel(Panel::new("a"))
        .separator(separator("sep").disabled(true))
        .panel(Panel::new("b"))
        .mount(ui.document(), &ui.by_id("host"))
        .unwrap();
    let separator = ui.by_id("sep");
    assert_eq!(
        attribute(&separator, "aria-disabled").as_deref(),
        Some("true")
    );
    assert_eq!(
        attribute(&separator, "data-separator").as_deref(),
        Some("disabled")
    );
    assert_eq!(attribute(&separator, "tabindex"), None);
    let (x, y) = center(&separator);
    mouse(&mut ui, MouseEventType::Down, x, y, MouseButton::Left);
    mouse(
        &mut ui,
        MouseEventType::Move,
        x + 80.0,
        y,
        MouseButton::Left,
    );
    mouse(&mut ui, MouseEventType::Up, x + 80.0, y, MouseButton::Left);
    assert_eq!(group.layout(), vec![50.0, 50.0]);
}
