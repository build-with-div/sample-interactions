//! Data view probes: sticky headers, measuring rows, the cost of long
//! lists with and without windowing, a column resize, and the
//! accessibility tree a grid, tree or listbox produces.
//!
//! Timings are wall-clock in the test profile (the app unoptimized, its
//! dependencies at opt-level 2, as Cargo.toml sets), single-threaded test
//! runner, on whatever machine runs them. They are printed, never asserted.

mod common;

use common::*;
use div::prelude::*;
use std::time::Instant;

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.0
}

#[test]
fn a_sticky_header_stays_at_the_top_of_its_scroller() {
    let ui = app(move || {
        let list = view! {
            <div id="list" style:height="200px" style:overflow-y="auto" style:width="300px">
                <div id="header" style:position="sticky" style:top="0" style:height="24px" style:background-color="rgb(255, 0, 0)">"Name"</div>
            </div>
        };
        if let ViewNode::Element(e) = &list {
            for i in 0..50 {
                mount_view(
                    e,
                    view! { <div style:height="24px">{format!("row {i}")}</div> },
                );
            }
        }
        list
    });
    let list = ui.by_id("list");
    let header = ui.by_id("header");
    let before = header.bounding_rect().unwrap().unwrap().y;
    list.scroll_to(0.0, 300.0).unwrap();
    ui.document().update_all().unwrap();
    let after = header.bounding_rect().unwrap().unwrap().y;
    let painted = pixel(&ui, 280, 12);
    report(
        "grid.sticky-header",
        format!("header y before scroll {before}, after scroll_to(300) {after}; pixel under it {painted:?}"),
    );
    assert_eq!(before, after);
    assert_eq!(painted, [255, 0, 0, 255]);
}

#[test]
fn row_heights_can_be_measured_after_layout() {
    let ui = app(move || {
        view! {
            <div style:width="200px" style:font-size="16px" style:line-height="20px">
                <div id="short">"one line"</div>
                <div id="long">"a row with enough words in it to wrap onto three lines here"</div>
            </div>
        }
    });
    let short = ui.by_id("short").bounding_rect().unwrap().unwrap().height;
    let long = ui.by_id("long").bounding_rect().unwrap().unwrap().height;
    report(
        "list.measure",
        format!("bounding_rect heights of a 1-line and a wrapping row at width 200px: {short}, {long}; no ResizeObserver to learn when they change"),
    );
    assert_eq!(short, 20.0);
    assert!(long >= 40.0);
}

fn mount_flat_rows(count: u32) -> (f64, f64) {
    let rows = create_signal((0..count).collect::<Vec<u32>>());
    let start = Instant::now();
    let ui = app(move || {
        let list = view! { <div id="list" style:height="400px" style:overflow-y="auto" style:width="400px"></div> };
        let items = For(
            move || rows.get(),
            |r| *r,
            |r| view! { <div style:height="24px">{format!("row {r}")}</div> },
        );
        if let ViewNode::Element(e) = &list {
            mount_view(e, items);
        }
        list
    });
    let mount = ms(start);
    let start = Instant::now();
    ui.document().render_to_bitmap().unwrap();
    (mount, ms(start))
}

#[test]
fn all_rows_mounted_cost() {
    let mut lines = Vec::new();
    for count in [1_000, 5_000] {
        let (mount, render) = mount_flat_rows(count);
        lines.push(format!(
            "{count} rows: mount {mount:.0} ms, first render {render:.0} ms"
        ));
    }
    report("list.unvirtualized", lines.join("; "));
}

/// Windowing by hand: a spacer as tall as all rows, and only the rows in
/// view (plus overscan) mounted, absolutely positioned. The scroll event
/// moves the window.
#[test]
fn windowed_list_scroll_cost() {
    const COUNT: u32 = 100_000;
    const ROW: f32 = 24.0;
    const VIEW: f32 = 480.0;
    let first = create_signal(0u32);
    let start = Instant::now();
    let mut ui = app(move || {
        let list = view! { <div id="list" style:height="480px" style:overflow-y="auto" style:width="400px" style:position="relative"></div> };
        let spacer = view! { <div id="spacer" style:width="1px"></div> };
        let visible = For(
            move || {
                let from = first.get();
                (from..(from + (VIEW / ROW) as u32 + 6).min(COUNT)).collect::<Vec<u32>>()
            },
            |r| *r,
            |r| {
                let row = view! { <div style:position="absolute" style:left="0" style:right="0" style:height="24px">{format!("row {r}")}</div> };
                if let ViewNode::Element(e) = &row {
                    e.set_top(div::typed_style::Length::px(r as f32 * ROW))
                        .unwrap();
                }
                row
            },
        );
        if let ViewNode::Element(e) = &list {
            mount_view(e, spacer);
            mount_view(e, visible);
        }
        list
    });
    let list = ui.by_id("list");
    ui.by_id("spacer")
        .set_height(LengthValue::px(COUNT as f32 * ROW))
        .unwrap();
    let scroller = list.clone();
    list.on("scroll", move |_| {
        let top = scroller.scroll_top().unwrap();
        first.set(((top as f32 / ROW) as u32).saturating_sub(3));
    })
    .unwrap();
    ui.document().render_to_bitmap().unwrap();
    let mount = ms(start);

    let steps = 50;
    let (mut rewindow, mut render) = (0.0, 0.0);
    for step in 1..=steps {
        let start = Instant::now();
        list.scroll_to(0.0, step as f64 * 997.0).unwrap();
        // Scroll events are queued for the next frame, as in Chromium;
        // TestApp::run_until_idle does not deliver them.
        ui.document().dispatch_pending_events().unwrap();
        ui.run_until_idle();
        rewindow += ms(start);
        let start = Instant::now();
        ui.document().render_to_bitmap().unwrap();
        render += ms(start);
    }
    let (rewindow, render) = (rewindow / steps as f64, render / steps as f64);
    let mounted = list.scroll_metrics().unwrap().map(|m| m.scroll_height);
    let shown = ui
        .find_text(&format!("row {}", (50.0 * 997.0 / ROW) as u32))
        .is_some();
    report(
        "list.windowed",
        format!("{COUNT} rows windowed by hand: mount+render {mount:.0} ms; per step of 997px (every row replaced) over {steps} steps: {rewindow:.1} ms to scroll and re-window (For swaps ~26 rows) + {render:.1} ms to lay out and render; scroll_height {mounted:?}; target row shown {shown}"),
    );
    assert!(shown);
}

#[test]
fn a_column_resize_relayouts_the_visible_grid() {
    const ROWS: u32 = 40;
    const COLS: usize = 8;
    let widths: [Signal<f32>; COLS] = std::array::from_fn(|_| create_signal(100.0f32));
    let ui = app(move || {
        let grid = view! { <div id="grid" style:width="800px"></div> };
        if let ViewNode::Element(e) = &grid {
            for r in 0..ROWS {
                let row = view! { <div style:display="flex" style:height="24px"></div> };
                if let ViewNode::Element(row_element) = &row {
                    for (c, &width) in widths.iter().enumerate() {
                        mount_view(
                            row_element,
                            view! {
                                <div style:flex-shrink="0" style:overflow="hidden" style:white-space="nowrap"
                                    style:width={LengthValue::px(width.get())}>{format!("r{r}c{c}")}</div>
                            },
                        );
                    }
                }
                mount_view(e, row);
            }
        }
        grid
    });
    ui.document().render_to_bitmap().unwrap();
    let steps = 20;
    let (mut set, mut render) = (0.0, 0.0);
    for step in 0..steps {
        let start = Instant::now();
        widths[2].set(100.0 + step as f32 * 3.0);
        set += ms(start);
        let start = Instant::now();
        ui.document().render_to_bitmap().unwrap();
        render += ms(start);
    }
    let (set, render) = (set / steps as f64, render / steps as f64);
    report(
        "grid.column-resize",
        format!("{ROWS} rows x {COLS} cells, one column's width signal (read by its {ROWS} cells) changed: {set:.1} ms to run the effects + {render:.1} ms to lay out and render, per change"),
    );
}

fn find_node<'a>(
    update: &'a div::AccessibilityTreeUpdate,
    label: &str,
) -> Option<&'a div::AccessibilityNode> {
    update
        .nodes
        .iter()
        .map(|(_, node)| node)
        .find(|node| node.label() == Some(label))
}

#[test]
fn aria_states_on_custom_widgets() {
    let ui = app(move || {
        view! {
            <div>
                <div id="tree" aria-label="files">
                    <div id="item" tabindex="0" aria-label="src" aria-expanded="true" aria-selected="true" aria-level="2"></div>
                </div>
                <div id="by-attribute" role="listbox" aria-label="by attribute"></div>
            </div>
        }
    });
    ui.by_id("tree")
        .set_accessibility_role(AccessibilityRole::Tree)
        .unwrap();
    ui.by_id("item")
        .set_accessibility_role(AccessibilityRole::TreeItem)
        .unwrap();
    let update = ui.document().accessibility_update().unwrap();
    let tree = find_node(&update, "files").map(|n| n.role());
    let item = find_node(&update, "src");
    let item_state = item.map(|n| (n.role(), n.is_expanded(), n.is_selected(), n.level()));
    let by_attribute = find_node(&update, "by attribute").map(|n| n.role());
    report(
        "a11y.custom-widgets",
        format!("set_accessibility_role(Tree) -> {tree:?}; treeitem with aria-expanded/selected/level=true/true/2 -> (role, expanded, selected, level) {item_state:?}; role=\"listbox\" attribute alone -> {by_attribute:?}"),
    );
    assert_eq!(tree, Some(AccessibilityRole::Tree));
    let (role, expanded, selected, level) = item_state.unwrap();
    assert_eq!(role, AccessibilityRole::TreeItem);
    assert_eq!((expanded, selected, level), (None, None, None));
    assert_ne!(by_attribute, Some(AccessibilityRole::ListBox));
}

/// The same grid with block rows and inline-block cells, to tell flex
/// layout cost apart from the cost of any relayout.
#[test]
fn a_column_resize_without_flex() {
    const ROWS: u32 = 40;
    const COLS: usize = 8;
    let widths: [Signal<f32>; COLS] = std::array::from_fn(|_| create_signal(100.0f32));
    let ui = app(move || {
        let grid = view! { <div id="grid" style:width="800px"></div> };
        if let ViewNode::Element(e) = &grid {
            for r in 0..ROWS {
                let row = view! { <div style:height="24px" style:white-space="nowrap"></div> };
                if let ViewNode::Element(row_element) = &row {
                    for (c, &width) in widths.iter().enumerate() {
                        mount_view(
                            row_element,
                            view! {
                                <div style:display="inline-block" style:overflow="hidden"
                                    style:width={LengthValue::px(width.get())}>{format!("r{r}c{c}")}</div>
                            },
                        );
                    }
                }
                mount_view(e, row);
            }
        }
        grid
    });
    ui.document().render_to_bitmap().unwrap();
    let steps = 20;
    let (mut set, mut render) = (0.0, 0.0);
    for step in 0..steps {
        let start = Instant::now();
        widths[2].set(100.0 + step as f32 * 3.0);
        set += ms(start);
        let start = Instant::now();
        ui.document().render_to_bitmap().unwrap();
        render += ms(start);
    }
    let (set, render) = (set / steps as f64, render / steps as f64);
    let start = Instant::now();
    ui.document().render_to_bitmap().unwrap();
    let unchanged = ms(start);
    report(
        "grid.column-resize-block",
        format!("same grid, block rows and inline-block cells: {set:.1} ms effects + {render:.1} ms layout and render per change; a render with nothing changed {unchanged:.1} ms"),
    );
}
