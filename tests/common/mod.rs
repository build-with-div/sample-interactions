//! Shared helpers for the probes. Each probe asserts what Div does today at
//! the pinned commit and prints one `PROBE <id>: <finding>` line, so
//! `cargo test -- --nocapture` reproduces the tables in docs/design.md. A
//! probe that starts failing after a Div bump means a finding changed: update
//! the design's matrix with it.

#![allow(dead_code)]

use div::prelude::*;
use div::testing::TestApp;
pub use div::PointerEventKind;
use std::cell::RefCell;
use std::rc::Rc;

pub const WIDTH: f64 = 800.0;
pub const HEIGHT: f64 = 600.0;

pub fn app<V: IntoView>(view: impl FnOnce() -> V + 'static) -> TestApp {
    TestApp::new(LogicalSize::new(WIDTH, HEIGHT)).mount(view)
}

pub fn report(id: &str, finding: impl AsRef<str>) {
    println!("PROBE {id}: {}", finding.as_ref());
}

/// Every event type a direct-manipulation widget might listen for.
pub const POINTER_TYPES: &[&str] = &[
    "mousedown",
    "mouseup",
    "mousemove",
    "click",
    "dblclick",
    "auxclick",
    "contextmenu",
    "pointerdown",
    "pointerup",
    "pointermove",
    "pointercancel",
    "gotpointercapture",
    "lostpointercapture",
];

/// A shared list of what listeners saw.
#[derive(Clone, Default)]
pub struct Log(Rc<RefCell<Vec<String>>>);

impl Log {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, entry: impl Into<String>) {
        self.0.borrow_mut().push(entry.into());
    }

    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.borrow_mut())
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.0.borrow().clone()
    }

    /// Log `label:type` for each of `types` reaching `element` (target or
    /// bubble phase).
    pub fn listen(&self, element: &Element, label: &str, types: &[&str]) {
        for event_type in types {
            let log = self.clone();
            let label = label.to_owned();
            element
                .on(event_type, move |event: &Event| {
                    log.push(format!("{label}:{}", event.event_type));
                })
                .expect("listener registers");
        }
    }

    /// Like `listen`, with the button and pointer id of each event.
    pub fn listen_detail(&self, element: &Element, label: &str, types: &[&str]) {
        for event_type in types {
            let log = self.clone();
            let label = label.to_owned();
            element
                .on(event_type, move |event: &Event| {
                    log.push(format!(
                        "{label}:{}(button={},id={})",
                        event.event_type, event.mouse_button, event.pointer_id
                    ));
                })
                .expect("listener registers");
        }
    }
}

pub fn center(element: &Element) -> (f32, f32) {
    let rect = element.bounding_rect().unwrap().expect("element has a box");
    (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
}

pub fn mouse(ui: &mut TestApp, kind: MouseEventType, x: f32, y: f32, button: MouseButton) {
    mouse_with(ui, kind, x, y, button, Modifiers::NONE);
}

pub fn mouse_with(
    ui: &mut TestApp,
    kind: MouseEventType,
    x: f32,
    y: f32,
    button: MouseButton,
    modifiers: Modifiers,
) {
    ui.document()
        .dispatch_mouse_event(kind, x, y, button, modifiers)
        .expect("mouse event dispatches");
    ui.run_until_idle();
}

/// A pointer event with DOM pointer names, as the native adapter sends for
/// touch (pointer id > 0).
pub fn pointer(ui: &mut TestApp, id: u64, kind: PointerEventKind, x: f32, y: f32) {
    ui.document()
        .dispatch_pointer_event(id, kind, x, y, MouseButton::Left, Modifiers::NONE, true)
        .expect("pointer event dispatches");
    ui.run_until_idle();
}

pub fn pixel(ui: &TestApp, x: u32, y: u32) -> [u8; 4] {
    let bitmap = ui.document().render_to_bitmap().unwrap();
    let width = (bitmap.pixels().len() / HEIGHT as usize / 4) as u32;
    let scale = width / WIDTH as u32;
    let i = (((y * scale) * width + x * scale) * 4) as usize;
    let p = bitmap.pixels();
    [p[i], p[i + 1], p[i + 2], p[i + 3]]
}
