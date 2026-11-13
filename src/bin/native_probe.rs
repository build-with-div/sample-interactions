//! Native input probe. Opens one real window and prints, one line each,
//! the platform events Div's App receives (`PLATFORM ...`) and the DOM
//! events listeners see (`EVENT ...`). `tools/native-input.ps1` drives it
//! on Windows by posting mouse messages to the window, then closes it.
//!
//! The handle (`#handle`, 100 x 100 at 50, 50) captures the pointer on
//! press, as a drag handle would.

use div::prelude::*;
use div::PlatformEvent;

fn id_of(element: Option<Element>) -> String {
    element
        .and_then(|e| e.get_attribute("id").ok().flatten())
        .unwrap_or_else(|| "-".into())
}

const TYPES: &[&str] = &[
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
    "wheel",
    "keydown",
];

fn main() -> Result<(), div::Error> {
    let mut app = App::builder()
        .title("div-native-probe")
        .size(LogicalSize::new(600.0, 400.0))
        .build()?;
    app.on_platform_event(|event| match event {
        PlatformEvent::Pointer {
            pointer_id,
            phase,
            x,
            y,
            button,
            ..
        } => println!("PLATFORM pointer id={pointer_id} {phase:?} {x:.1},{y:.1} {button:?}"),
        PlatformEvent::Wheel {
            x,
            y,
            delta_x,
            delta_y,
            ..
        } => println!("PLATFORM wheel {x:.1},{y:.1} delta={delta_x},{delta_y}"),
        PlatformEvent::Resized(viewport) => println!("PLATFORM resized {viewport:?}"),
        PlatformEvent::DroppedFile(path) => println!("PLATFORM dropped-file {}", path.display()),
        PlatformEvent::HoveredFile(path) => println!("PLATFORM hovered-file {}", path.display()),
        PlatformEvent::HoveredFileCancelled => println!("PLATFORM hovered-file-cancelled"),
        PlatformEvent::CloseRequested => println!("PLATFORM close-requested"),
        _ => {}
    });
    app.run(|| {
        let root = view! {
            <div id="root" style:position="absolute" style:top="0" style:left="0" style:width="600px" style:height="400px" style:background-color="rgb(240, 240, 240)">
                <div id="handle" style:position="absolute" style:top="50px" style:left="50px" style:width="100px" style:height="100px" style:background-color="rgb(0, 0, 255)"></div>
                <div id="other" style:position="absolute" style:top="50px" style:left="300px" style:width="100px" style:height="100px" style:background-color="rgb(0, 128, 0)"></div>
                <div id="scroller" style:position="absolute" style:top="200px" style:left="300px" style:width="200px" style:height="150px" style:overflow-y="auto" style:background-color="rgb(255, 255, 255)">
                    <div style:height="2000px"></div>
                </div>
            </div>
        };
        if let ViewNode::Element(root) = &root {
            for event_type in TYPES {
                root.on(event_type, |event: &Event| {
                    println!(
                        "EVENT {} target={} button={} pointer={} at={:.1},{:.1} delta={},{} mods={}",
                        event.event_type,
                        id_of(event.target()),
                        event.mouse_button,
                        event.pointer_id,
                        event.mouse_x,
                        event.mouse_y,
                        event.delta_x,
                        event.delta_y,
                        event.modifiers
                    );
                })
                .unwrap();
            }
            // The scroller (at 300,200, 200 x 150) starts scrolled to 500, so a
            // wheel notch either way shows its direction.
            let scroller = root.first_child().unwrap().unwrap().next_sibling().unwrap().unwrap().next_sibling().unwrap().unwrap();
            let watched = scroller.clone();
            scroller
                .on("scroll", move |_| {
                    println!("SCROLL scroller top={}", watched.scroll_top().unwrap());
                })
                .unwrap();
            let start = scroller.clone();
            div::task::spawn_local(async move {
                start.scroll_to(0.0, 500.0).unwrap();
                println!("SCROLL scroller start top={}", start.scroll_top().unwrap());
            })
            .detach();
            let handle = root.first_child().unwrap().unwrap();
            for event_type in ["mousedown", "pointerdown"] {
                let target = handle.clone();
                handle
                    .on(event_type, move |event: &Event| {
                        let result = target.set_pointer_capture(event.pointer_id);
                        println!("CAPTURE pointer={} {result:?}", event.pointer_id);
                    })
                    .unwrap();
            }
        }
        root
    })
}
