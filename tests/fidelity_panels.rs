//! The fidelity runner for the react-resizable-panels port: the scripted
//! scenarios in tests/fixtures/fidelity/panels-scenarios.json, replayed on
//! Div and compared with the package itself in pinned Chromium
//! (panels-chromium.json, written by tools/fidelity/capture.mjs).
//!
//! After mount and after every step, each group, panel and separator is
//! compared: its box (`getBoundingClientRect`, exactly), its attributes,
//! the focused element, the document cursor, the layout callbacks and the
//! click targets. The differences must be exactly those listed, value for
//! value, in tests/fixtures/fidelity/panels-div-differences.txt, each
//! under the gap that owns it: a new difference fails, and so does a
//! listed one that changed or no longer occurs. The list is the measured
//! gap between the two at this Div pin.

mod common;

use common::*;
use div::prelude::*;
use sample_interactions::panels::{
    document_cursor, Group, GroupHandle, Orientation, Panel, Separator,
};
use serde_json::{json, Map, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

const SCENARIOS: &str = include_str!("fixtures/fidelity/panels-scenarios.json");
const CHROMIUM: &str = include_str!("fixtures/fidelity/panels-chromium.json");
const KNOWN_DIFFERENCES: &str = include_str!("fixtures/fidelity/panels-div-differences.txt");

/// Attributes read on Div besides those Chromium shows, so that an
/// attribute only the port sets is a difference too (Div cannot list an
/// element's attributes).
const ATTRIBUTES: &[&str] = &[
    "id",
    "data-group",
    "data-panel",
    "data-separator",
    "data-testid",
    "data-disabled",
    "role",
    "tabindex",
    "aria-orientation",
    "aria-controls",
    "aria-valuemin",
    "aria-valuemax",
    "aria-valuenow",
    "aria-disabled",
];

/// What the callbacks and the click listener saw since the last snapshot.
#[derive(Clone, Default)]
struct Recorder {
    changes: Rc<RefCell<Vec<Value>>>,
    changed: Rc<RefCell<Vec<Value>>>,
    clicks: Rc<RefCell<Vec<String>>>,
}

fn size_of(value: &Value) -> Option<sample_interactions::panels::Size> {
    match value {
        Value::String(text) => Some(text.as_str().into()),
        Value::Number(number) => Some(number.as_f64().unwrap().into()),
        _ => None,
    }
}

/// Mount `spec` (a group) into `parent`, then its nested groups, as the
/// reference page renders it.
fn mount(
    ui: &div::testing::TestApp,
    spec: &Value,
    parent: &Element,
    separator_size: f32,
    recorder: &Recorder,
    handles: &mut Vec<GroupHandle>,
) {
    let id = spec["id"].as_str().unwrap().to_owned();
    let orientation = match spec["orientation"].as_str().unwrap() {
        "horizontal" => Orientation::Horizontal,
        _ => Orientation::Vertical,
    };
    let mut group = Group::new(&id, orientation);
    let mut nested = Vec::new();
    for child in spec["children"].as_array().unwrap() {
        if let Some(separator) = child.get("separator") {
            let disabled = separator["disabled"].as_bool().unwrap_or(false);
            group = group.separator(
                Separator::new(separator["id"].as_str().unwrap())
                    .disabled(disabled)
                    .style(move |element| {
                        let size = LengthValue::px(separator_size);
                        match orientation {
                            Orientation::Horizontal => element.set_width(size).unwrap(),
                            Orientation::Vertical => element.set_height(size).unwrap(),
                        }
                    }),
            );
            continue;
        }
        let spec = &child["panel"];
        let panel_id = spec["id"].as_str().unwrap();
        let mut panel = Panel::new(panel_id);
        if let Some(size) = size_of(&spec["defaultSize"]) {
            panel = panel.default_size(size);
        }
        if let Some(size) = size_of(&spec["minSize"]) {
            panel = panel.min_size(size);
        }
        if let Some(size) = size_of(&spec["maxSize"]) {
            panel = panel.max_size(size);
        }
        if let Some(size) = size_of(&spec["collapsedSize"]) {
            panel = panel.collapsed_size(size);
        }
        if let Some(collapsible) = spec["collapsible"].as_bool() {
            panel = panel.collapsible(collapsible);
        }
        if let Some(group) = spec.get("group") {
            nested.push((panel_id.to_owned(), group.clone()));
        }
        group = group.panel(panel);
    }
    let changes = recorder.changes.clone();
    let changed = recorder.changed.clone();
    let (change_id, changed_id) = (id.clone(), id.clone());
    let handle = group
        .on_layout_change(move |layout| {
            changes
                .borrow_mut()
                .push(json!({ "group": change_id, "layout": layout }))
        })
        .on_layout_changed(move |layout| {
            changed
                .borrow_mut()
                .push(json!({ "group": changed_id, "layout": layout }))
        })
        .mount(ui.document(), parent)
        .expect("the group mounts");
    for (panel_id, spec) in nested {
        let content = handle.panel_content(&panel_id).unwrap().clone();
        mount(ui, &spec, &content, separator_size, recorder, handles);
    }
    handles.push(handle);
}

fn element_name(element: &Element) -> String {
    match element.get_attribute("id").unwrap() {
        Some(id) => id,
        None => format!("{:?}", element.kind().unwrap()).to_lowercase(),
    }
}

/// The page's `snapshot()`, read from Div.
fn snapshot(
    ui: &div::testing::TestApp,
    host: &Element,
    recorder: &Recorder,
    chromium: &Value,
) -> Value {
    let mut elements = Vec::new();
    let mut stack = vec![host.clone()];
    // Document order: depth first, children in order.
    while let Some(element) = stack.pop() {
        let mut children = Vec::new();
        let mut child = element.first_child().unwrap();
        while let Some(current) = child {
            child = current.next_sibling().unwrap();
            children.push(current);
        }
        stack.extend(children.into_iter().rev());

        let has = |name: &str| element.get_attribute(name).unwrap().is_some();
        let kind = if has("data-group") {
            "group"
        } else if has("data-panel") {
            "panel"
        } else if has("data-separator") {
            "separator"
        } else {
            continue;
        };
        let id = element.get_attribute("id").unwrap().unwrap_or_default();
        let rect = element.bounding_rect().unwrap().expect("a box");
        let mut names: Vec<String> = ATTRIBUTES.iter().map(|name| name.to_string()).collect();
        // Attributes Chromium shows on the element with this id.
        if let Some(reference) = chromium["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == id.as_str())
        {
            names.extend(reference["attributes"].as_object().unwrap().keys().cloned());
        }
        let mut attributes = Map::new();
        for name in names {
            if let Some(value) = element.get_attribute(&name).unwrap() {
                attributes.insert(name, Value::String(value));
            }
        }
        elements.push(json!({
            "id": id,
            "kind": kind,
            "rect": [rect.x as f64, rect.y as f64, rect.width as f64, rect.height as f64],
            "attributes": attributes,
        }));
    }
    let document = ui.document();
    let body = document.body();
    let focus = document
        .focused_element()
        .unwrap()
        .filter(|element| !element.is_same_node(&body).unwrap())
        .map(|element| element_name(&element));
    json!({
        "elements": elements,
        "focus": focus,
        "cursor": document_cursor(document).unwrap_or("auto"),
        "changes": std::mem::take(&mut *recorder.changes.borrow_mut()),
        "changed": std::mem::take(&mut *recorder.changed.borrow_mut()),
        "clicks": std::mem::take(&mut *recorder.clicks.borrow_mut()),
    })
}

fn key(name: &str) -> Key {
    match name {
        "ArrowLeft" => Key::ArrowLeft,
        "ArrowRight" => Key::ArrowRight,
        "ArrowUp" => Key::ArrowUp,
        "ArrowDown" => Key::ArrowDown,
        "Home" => Key::Home,
        "End" => Key::End,
        "Enter" => Key::Enter,
        "F6" => Key::Function(6),
        other => panic!("unknown key {other}"),
    }
}

fn step(ui: &mut div::testing::TestApp, entry: &Value) {
    if let Some(kind) = entry["mouse"].as_str() {
        let kind = match kind {
            "move" => MouseEventType::Move,
            "down" => MouseEventType::Down,
            _ => MouseEventType::Up,
        };
        let x = entry["x"].as_f64().unwrap() as f32;
        let y = entry["y"].as_f64().unwrap() as f32;
        mouse(ui, kind, x, y, MouseButton::Left);
    } else if let Some(name) = entry["key"].as_str() {
        let modifiers = if entry["shift"].as_bool().unwrap_or(false) {
            Modifiers::SHIFT
        } else {
            Modifiers::NONE
        };
        ui.press_with(modifiers, key(name));
    } else if let Some(id) = entry["focus"].as_str() {
        // `focus()` on an element that can't take focus does nothing on
        // the web; Div reports it as an error, which the step ignores.
        // (`div` doesn't export the engine's error type.)
        match ui.by_id(id).focus() {
            Ok(()) => {}
            Err(error) if format!("{error:?}") == "Engine(NotFocusable)" => {}
            Err(error) => panic!("focus {id}: {error:?}"),
        }
        ui.run_until_idle();
    } else {
        panic!("unknown step {entry}");
    }
}

/// Bitwise, so that an f32 box can't pass for a LayoutUnit one.
fn same_number(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(a), Some(b)) => a.to_bits() == b.to_bits() || (a == 0.0 && b == 0.0),
        _ => a == b,
    }
}

fn same_numbers(a: &Value, b: &Value) -> bool {
    match (a.as_array(), b.as_array()) {
        (Some(a), Some(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_number(a, b)),
        _ => a == b,
    }
}

/// Callbacks per group in call order. React runs a child's effects before
/// its parent's, so the order across groups is the reconciler's, not the
/// package's; it isn't compared.
fn per_group(calls: &Value) -> BTreeMap<String, Vec<Value>> {
    let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for call in calls.as_array().unwrap() {
        groups
            .entry(call["group"].as_str().unwrap().to_owned())
            .or_default()
            .push(call["layout"].clone());
    }
    groups
}

fn same_calls(a: &Value, b: &Value) -> bool {
    let (a, b) = (per_group(a), per_group(b));
    a.len() == b.len()
        && a.iter().zip(&b).all(|((group_a, a), (group_b, b))| {
            group_a == group_b
                && a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| same_numbers(a, b))
        })
}

#[derive(Default)]
struct Tally {
    checked: BTreeMap<&'static str, (usize, usize)>,
    differences: Vec<(String, usize, &'static str, String)>,
}

impl Tally {
    fn check(
        &mut self,
        scenario: &str,
        index: usize,
        field: &'static str,
        ok: bool,
        detail: impl FnOnce() -> String,
    ) {
        let entry = self.checked.entry(field).or_default();
        entry.1 += 1;
        if ok {
            entry.0 += 1;
        } else {
            self.differences
                .push((scenario.to_owned(), index, field, detail()));
        }
    }
}

fn compare(tally: &mut Tally, scenario: &str, index: usize, div: &Value, chromium: &Value) {
    let ids = |snapshot: &Value| -> Vec<String> {
        snapshot["elements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|element| {
                format!(
                    "{}:{}",
                    element["kind"].as_str().unwrap(),
                    element["id"].as_str().unwrap()
                )
            })
            .collect()
    };
    let (div_ids, chromium_ids) = (ids(div), ids(chromium));
    tally.check(scenario, index, "elements", div_ids == chromium_ids, || {
        format!("Div {div_ids:?}, Chromium {chromium_ids:?}")
    });
    for (ours, theirs) in div["elements"]
        .as_array()
        .unwrap()
        .iter()
        .zip(chromium["elements"].as_array().unwrap())
    {
        let id = theirs["id"].as_str().unwrap();
        tally.check(
            scenario,
            index,
            "box",
            same_numbers(&ours["rect"], &theirs["rect"]),
            || format!("{id}: Div {}, Chromium {}", ours["rect"], theirs["rect"]),
        );
        tally.check(
            scenario,
            index,
            "attributes",
            ours["attributes"] == theirs["attributes"],
            || {
                let (ours, theirs) = (
                    ours["attributes"].as_object().unwrap(),
                    theirs["attributes"].as_object().unwrap(),
                );
                let mut names: Vec<&String> = ours.keys().chain(theirs.keys()).collect();
                names.sort();
                names.dedup();
                let differing: Vec<String> = names
                    .into_iter()
                    .filter(|name| ours.get(*name) != theirs.get(*name))
                    .map(|name| {
                        format!(
                            "{name}: Div {:?}, Chromium {:?}",
                            ours.get(name),
                            theirs.get(name)
                        )
                    })
                    .collect();
                format!("{id}: {}", differing.join("; "))
            },
        );
    }
    for field in ["focus", "cursor", "clicks"] {
        tally.check(
            scenario,
            index,
            field_name(field),
            div[field] == chromium[field],
            || format!("Div {}, Chromium {}", div[field], chromium[field]),
        );
    }
    for field in ["changes", "changed"] {
        tally.check(
            scenario,
            index,
            field_name(field),
            same_calls(&div[field], &chromium[field]),
            || format!("Div {}, Chromium {}", div[field], chromium[field]),
        );
    }
}

fn field_name(field: &str) -> &'static str {
    match field {
        "focus" => "focus",
        "cursor" => "cursor",
        "clicks" => "clicks",
        "changes" => "onLayoutChange",
        "changed" => "onLayoutChanged",
        _ => unreachable!(),
    }
}

#[test]
fn panels_match_the_package_in_chromium() {
    let spec: Value = serde_json::from_str(SCENARIOS).unwrap();
    let chromium: Value = serde_json::from_str(CHROMIUM).unwrap();
    let separator_size = spec["separatorSize"].as_f64().unwrap() as f32;
    let (width, height) = (
        spec["host"]["width"].as_f64().unwrap(),
        spec["host"]["height"].as_f64().unwrap(),
    );
    let mut tally = Tally::default();
    let mut snapshots = 0;

    for scenario in spec["scenarios"].as_array().unwrap() {
        let id = scenario["id"].as_str().unwrap();
        let reference = chromium["scenarios"][id]
            .as_array()
            .expect("Chromium captured the scenario");
        let mut ui = app(move || {
            view! {
                <div id="host" style:width=LengthValue::px(width as f32) style:height=LengthValue::px(height as f32)></div>
            }
        });
        let host = ui.by_id("host");
        let recorder = Recorder::default();
        // Registered before the group, as the page registers it before
        // React renders.
        let clicks = recorder.clicks.clone();
        ui.document()
            .body()
            .on_capture("click", move |event: &Event| {
                let target = event.target().map(|element| element_name(&element));
                clicks.borrow_mut().push(target.unwrap_or_default());
            })
            .unwrap();
        let mut handles = Vec::new();
        mount(
            &ui,
            &scenario["group"],
            &host,
            separator_size,
            &recorder,
            &mut handles,
        );
        ui.run_until_idle();

        let steps = scenario["steps"].as_array().unwrap();
        assert_eq!(
            reference.len(),
            steps.len() + 1,
            "{id}: one snapshot per step"
        );
        for index in 0..reference.len() {
            if index > 0 {
                step(&mut ui, &steps[index - 1]);
            }
            let ours = snapshot(&ui, &host, &recorder, &reference[index]);
            compare(&mut tally, id, index, &ours, &reference[index]);
            snapshots += 1;
        }
    }

    let browser = chromium["chromium"].as_str().unwrap();
    for (field, (matched, total)) in &tally.checked {
        println!("FIDELITY panels {field}: {matched}/{total} match {browser}");
    }
    println!(
        "FIDELITY panels: {snapshots} snapshots, {} differences",
        tally.differences.len()
    );
    let found: Vec<String> = tally
        .differences
        .iter()
        .map(|(scenario, index, field, detail)| format!("{scenario}#{index} {field}: {detail}"))
        .collect();

    // The list: `## <gap>` headings, then one difference per line.
    let mut listed = Vec::new();
    let mut gap = None;
    let mut per_gap: BTreeMap<String, usize> = BTreeMap::new();
    for line in KNOWN_DIFFERENCES.lines().map(str::trim_end) {
        if let Some(heading) = line.strip_prefix("## ") {
            gap = Some(heading.split_whitespace().next().unwrap().to_owned());
        } else if !line.is_empty() && !line.starts_with('#') {
            let gap = gap
                .clone()
                .expect("every difference is under a gap heading");
            *per_gap.entry(gap).or_default() += 1;
            listed.push(line.to_owned());
        }
    }
    for (gap, count) in &per_gap {
        println!("FIDELITY panels differences owned by {gap}: {count}");
    }
    let unexplained: Vec<&String> = found.iter().filter(|line| !listed.contains(line)).collect();
    let resolved: Vec<&String> = listed.iter().filter(|line| !found.contains(line)).collect();
    for line in &unexplained {
        println!("DIFF (not listed) {line}");
    }
    for line in &resolved {
        println!("DIFF (listed, no longer occurs) {line}");
    }
    assert!(
        unexplained.is_empty() && resolved.is_empty(),
        "{} differences are not in panels-div-differences.txt and {} listed ones no longer occur",
        unexplained.len(),
        resolved.len()
    );
}
