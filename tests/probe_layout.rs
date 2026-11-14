//! Layout probes: what the ports' layout depends on, measured on Div
//! without the ports.

mod common;

use common::*;
use div::prelude::*;
use serde_json::Value;

const CHROMIUM: &str = include_str!("fixtures/fidelity/flex-grow-chromium.json");

/// tools/fidelity/flex-grow-probe.html: `[id, row?, flex-grow of each item]`.
const CASES: &[(&str, bool, &[f32])] = &[
    ("row-30-45-25", true, &[30.0, 45.0, 25.0]),
    ("row-10-70-20", true, &[10.0, 70.0, 20.0]),
    ("row-25-55-20", true, &[25.0, 55.0, 20.0]),
    ("row-35-65", true, &[35.0, 65.0]),
    ("column-69.697-30.303", false, &[69.697, 30.303]),
    ("column-85-15", false, &[85.0, 15.0]),
];

/// The flex items react-resizable-panels lays out: `flex: <percent> 1 0px`
/// items between 4px `flex: 0 0 auto` separators, in an 800x400 box.
/// Returns `[offset, size]` along the main axis for every child.
fn boxes(ui: &div::testing::TestApp, row: bool, grows: &[f32]) -> Vec<[f64; 2]> {
    let document = ui.document();
    let group = Element::create(document, "div").unwrap();
    group.set_display(Display::Flex).unwrap();
    group
        .set_flex_direction(if row {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        })
        .unwrap();
    group.set_width(LengthValue::px(800.0)).unwrap();
    group.set_height(LengthValue::px(400.0)).unwrap();
    for (index, grow) in grows.iter().enumerate() {
        if index > 0 {
            let separator = Element::create(document, "div").unwrap();
            let size = LengthValue::px(4.0);
            if row {
                separator.set_width(size).unwrap();
            } else {
                separator.set_height(size).unwrap();
            }
            separator.set_flex_grow(0.0).unwrap();
            separator.set_flex_shrink(0.0).unwrap();
            separator.set_flex_basis(LengthValue::auto()).unwrap();
            group.append_child(&separator).unwrap();
        }
        let item = Element::create(document, "div").unwrap();
        item.set_flex_grow(*grow).unwrap();
        item.set_flex_shrink(1.0).unwrap();
        item.set_flex_basis(LengthValue::px(0.0)).unwrap();
        group.append_child(&item).unwrap();
    }
    ui.by_id("host").append_child(&group).unwrap();
    let origin = group.bounding_rect().unwrap().unwrap();
    let mut boxes = Vec::new();
    let mut child = group.first_child().unwrap();
    while let Some(current) = child {
        let rect = current.bounding_rect().unwrap().unwrap();
        boxes.push(if row {
            [(rect.x - origin.x) as f64, rect.width as f64]
        } else {
            [(rect.y - origin.y) as f64, rect.height as f64]
        });
        child = current.next_sibling().unwrap();
    }
    group.remove().unwrap();
    boxes
}

#[test]
fn fractional_flex_grow_rounds_unlike_chromium() {
    let ui = app(|| view! { <div id="host"></div> });
    let chromium: Value = serde_json::from_str(CHROMIUM).unwrap();
    let mut differing = Vec::new();
    let mut matched = 0;
    let mut total = 0;
    let mut largest: f64 = 0.0;
    for (id, row, grows) in CASES {
        let ours = boxes(&ui, *row, grows);
        let theirs: Vec<[f64; 2]> = serde_json::from_value(chromium["cases"][id].clone()).unwrap();
        assert_eq!(ours.len(), theirs.len(), "{id}");
        for (a, b) in ours.iter().zip(&theirs) {
            largest = largest.max((a[0] - b[0]).abs()).max((a[1] - b[1]).abs());
        }
        let same = ours.iter().zip(&theirs).filter(|(a, b)| a == b).count();
        matched += same;
        total += ours.len();
        if same != ours.len() {
            println!("  {id}: Div {ours:?}, Chromium {theirs:?}");
            differing.push(*id);
        }
    }
    report(
        "layout.flex-grow-fractions",
        format!(
            "{matched}/{total} boxes ([offset, size]) equal {} for fractional flex-grow; \
             {} of {} cases differ, by at most {largest} px",
            chromium["chromium"].as_str().unwrap(),
            differing.len(),
            CASES.len()
        ),
    );
    // One LayoutUnit: both engines snap to 1/64 px and round differently.
    assert_eq!(largest, 1.0 / 64.0);
    assert_eq!(
        differing,
        [
            "row-30-45-25",
            "row-10-70-20",
            "row-25-55-20",
            "row-35-65",
            "column-69.697-30.303",
            "column-85-15"
        ]
    );
}
