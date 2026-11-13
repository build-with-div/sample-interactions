//! Direct manipulation built by hand on today's API: a splitter (pointer
//! and keyboard), a cross-list reorder with a drag threshold and a ghost,
//! and rubber-band selection. Each shows the workaround works headlessly
//! and how much code the app has to own.

mod common;

use common::*;
use div::prelude::*;
use div::testing::Key;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

#[test]
fn a_splitter_by_hand() {
    let left_width = create_signal(200.0f32);
    let mut ui = app(move || {
        view! {
            <div id="split" style:display="flex" style:width="800px" style:height="400px">
                <div id="left" style:flex-shrink="0" style:width={LengthValue::px(left_width.get())} style:background-color="rgb(230, 230, 230)"></div>
                <div id="bar" tabindex="0" style:width="6px" style:flex-shrink="0" style:background-color="rgb(120, 120, 120)"></div>
                <div id="right" style:flex-grow="1"></div>
            </div>
        }
    });
    let bar = ui.by_id("bar");
    // div-gap: no `cursor: col-resize` (Div's Cursor has auto, default,
    // pointer, text, move, not-allowed); no aria-valuenow mapping.
    bar.set_accessibility_role(AccessibilityRole::Splitter)
        .unwrap();
    let dragging = Rc::new(Cell::new(false));
    let split_left = ui.by_id("split").bounding_rect().unwrap().unwrap().x;
    {
        let (bar2, dragging) = (bar.clone(), dragging.clone());
        bar.on("mousedown", move |event: &Event| {
            dragging.set(true);
            bar2.set_pointer_capture(event.pointer_id).unwrap();
        })
        .unwrap();
    }
    {
        let dragging = dragging.clone();
        bar.on("mousemove", move |event: &Event| {
            if dragging.get() {
                left_width.set((event.mouse_x - split_left - 3.0).clamp(100.0, 600.0));
            }
        })
        .unwrap();
    }
    {
        let (bar2, dragging) = (bar.clone(), dragging.clone());
        bar.on("mouseup", move |event: &Event| {
            dragging.set(false);
            // div-gap: capture is not released implicitly after the up.
            bar2.release_pointer_capture(event.pointer_id).unwrap();
        })
        .unwrap();
    }
    bar.on("keydown", move |event: &Event| {
        let step = match event.key() {
            Some(Key::ArrowLeft) => -10.0,
            Some(Key::ArrowRight) => 10.0,
            _ => return,
        };
        event.prevent_default();
        left_width.update(|w| *w = (*w + step).clamp(100.0, 600.0));
    })
    .unwrap();

    let (x, y) = center(&bar);
    mouse(&mut ui, MouseEventType::Down, x, y, MouseButton::Left);
    let start = Instant::now();
    let moves = 30;
    for i in 1..=moves {
        mouse(
            &mut ui,
            MouseEventType::Move,
            x + i as f32 * 5.0,
            y + 50.0,
            MouseButton::Left,
        );
        ui.document().render_to_bitmap().unwrap();
    }
    let per_move = start.elapsed().as_secs_f64() * 1000.0 / moves as f64;
    mouse(
        &mut ui,
        MouseEventType::Up,
        x + 150.0,
        y + 50.0,
        MouseButton::Left,
    );
    let after_drag = ui.by_id("left").bounding_rect().unwrap().unwrap().width;
    bar.focus().unwrap();
    ui.press(Key::ArrowLeft);
    ui.press(Key::ArrowLeft);
    let after_keys = ui.by_id("left").bounding_rect().unwrap().unwrap().width;
    report(
        "split.by-hand",
        format!("drag the bar 150px: left pane {after_drag}px; ArrowLeft x2: {after_keys}px; {per_move:.1} ms per drag move incl. render (30 moves)"),
    );
    assert_eq!(after_drag, 350.0);
    assert_eq!(after_keys, 330.0);
}

#[test]
fn a_cross_list_reorder_with_threshold_and_ghost() {
    let lists = create_signal(vec![
        vec!["a1".to_string(), "a2".into(), "a3".into()],
        vec!["b1".to_string(), "b2".into()],
    ]);
    let mut ui = app(move || {
        let board = view! { <div id="board" style:position="relative" style:width="800px" style:height="600px"></div> };
        if let ViewNode::Element(board_element) = &board {
            for (column, left) in [(0usize, 0.0f32), (1, 300.0)] {
                let list = view! { <div id={format!("list{column}")} data-list={column.to_string()} style:position="absolute" style:top="0" style:width="200px" style:height="400px" style:background-color="rgb(245, 245, 245)"></div> };
                if let ViewNode::Element(list_element) = &list {
                    list_element
                        .set_left(div::typed_style::Length::px(left))
                        .unwrap();
                    let cards = For(
                        move || lists.get()[column].clone(),
                        |card| card.clone(),
                        |card| view! { <div id={card.clone()} data-card={card.clone()} style:height="40px" style:margin="4px" style:background-color="rgb(255, 255, 255)">{card.clone()}</div> },
                    );
                    mount_view(list_element, cards);
                }
                mount_view(board_element, list);
            }
            let ghost = view! { <div id="ghost" style:position="fixed" style:display="none" style:pointer-events="none" style:width="200px" style:height="40px" style:opacity="0.6" style:background-color="rgb(0, 0, 255)"></div> };
            mount_view(board_element, ghost);
        }
        board
    });

    // App-owned drag state: where the press started, which card, whether
    // the threshold was passed.
    #[derive(Clone, Default)]
    struct Drag {
        start: Option<(f32, f32)>,
        card: Option<String>,
        active: bool,
    }
    let drag = Rc::new(std::cell::RefCell::new(Drag::default()));
    let board = ui.by_id("board");
    let ghost = ui.by_id("ghost");
    let document = ui.document().clone();
    {
        let (drag, board2) = (drag.clone(), board.clone());
        board
            .on("mousedown", move |event: &Event| {
                let card = event
                    .target()
                    .and_then(|t| t.get_attribute("data-card").ok().flatten());
                *drag.borrow_mut() = Drag {
                    start: Some((event.mouse_x, event.mouse_y)),
                    card,
                    active: false,
                };
                board2.set_pointer_capture(event.pointer_id).unwrap();
            })
            .unwrap();
    }
    {
        let (drag, ghost) = (drag.clone(), ghost.clone());
        board
            .on("mousemove", move |event: &Event| {
                let mut state = drag.borrow_mut();
                let (Some((sx, sy)), Some(_)) = (state.start, state.card.as_ref()) else {
                    return;
                };
                if !state.active && (event.mouse_x - sx).hypot(event.mouse_y - sy) < 4.0 {
                    return;
                }
                state.active = true;
                ghost.apply_style(&style! { display: block }).unwrap();
                ghost
                    .set_left(div::typed_style::Length::px(event.mouse_x - 100.0))
                    .unwrap();
                ghost
                    .set_top(div::typed_style::Length::px(event.mouse_y - 20.0))
                    .unwrap();
            })
            .unwrap();
    }
    {
        let (drag, board2, ghost) = (drag.clone(), board.clone(), ghost.clone());
        board
            .on("mouseup", move |event: &Event| {
                board2.release_pointer_capture(event.pointer_id).unwrap();
                ghost.apply_style(&style! { display: none }).unwrap();
                let state = std::mem::take(&mut *drag.borrow_mut());
                let (true, Some(card)) = (state.active, state.card) else {
                    return;
                };
                // Drop target by hit test: the ghost has pointer-events:
                // none, so the element under the pointer is a card or list.
                let mut under = document.hit_test(event.mouse_x, event.mouse_y).unwrap();
                let mut before: Option<String> = None;
                let mut column: Option<usize> = None;
                while let Some(element) = under {
                    if before.is_none() {
                        before = element.get_attribute("data-card").unwrap();
                    }
                    if let Some(list) = element.get_attribute("data-list").unwrap() {
                        column = list.parse().ok();
                        break;
                    }
                    under = element.parent().unwrap();
                }
                let Some(column) = column else { return };
                lists.update(|lists| {
                    for list in lists.iter_mut() {
                        list.retain(|c| *c != card);
                    }
                    let at = before
                        .and_then(|b| lists[column].iter().position(|c| *c == b))
                        .unwrap_or(lists[column].len());
                    lists[column].insert(at, card.clone());
                });
            })
            .unwrap();
    }

    // A 2px wiggle below the threshold is a click, not a drag.
    let (ax, ay) = center(&ui.by_id("a1"));
    mouse(&mut ui, MouseEventType::Down, ax, ay, MouseButton::Left);
    mouse(
        &mut ui,
        MouseEventType::Move,
        ax + 2.0,
        ay,
        MouseButton::Left,
    );
    mouse(&mut ui, MouseEventType::Up, ax + 2.0, ay, MouseButton::Left);
    let after_wiggle = lists.get();
    // Drag a1 onto b2: it lands before b2.
    let (bx, by) = center(&ui.by_id("b2"));
    mouse(&mut ui, MouseEventType::Down, ax, ay, MouseButton::Left);
    mouse(
        &mut ui,
        MouseEventType::Move,
        ax + 20.0,
        ay + 10.0,
        MouseButton::Left,
    );
    let ghost_shown = ghost.bounding_rect().unwrap().is_some();
    mouse(&mut ui, MouseEventType::Move, bx, by, MouseButton::Left);
    mouse(&mut ui, MouseEventType::Up, bx, by, MouseButton::Left);
    let after_drag = lists.get();
    report(
        "dnd.cross-list",
        format!("wiggle under threshold: {after_wiggle:?}; drag a1 onto b2 with a ghost (shown: {ghost_shown}): {after_drag:?}"),
    );
    assert_eq!(after_wiggle[0], ["a1", "a2", "a3"]);
    assert_eq!(after_drag, vec![vec!["a2", "a3"], vec!["b1", "a1", "b2"]]);
}

#[test]
fn rubber_band_selection_by_hand() {
    let mut ui = app(move || {
        let area = view! { <div id="area" style:position="relative" style:width="600px" style:height="400px"></div> };
        if let ViewNode::Element(e) = &area {
            for i in 0..12 {
                let (col, row) = (i % 4, i / 4);
                let tile = view! { <div id={format!("t{i}")} data-tile="1" style:position="absolute" style:width="80px" style:height="80px" style:background-color="rgb(200, 200, 200)"></div> };
                if let ViewNode::Element(t) = &tile {
                    t.set_left(div::typed_style::Length::px(10.0 + col as f32 * 100.0))
                        .unwrap();
                    t.set_top(div::typed_style::Length::px(10.0 + row as f32 * 100.0))
                        .unwrap();
                }
                mount_view(e, tile);
            }
            mount_view(
                e,
                view! { <div id="band" style:position="absolute" style:display="none" style:pointer-events="none" style:background-color="rgba(0, 0, 255, 0.2)"></div> },
            );
        }
        area
    });
    let area = ui.by_id("area");
    let tiles: Vec<Element> = (0..12).map(|i| ui.by_id(&format!("t{i}"))).collect();
    let start = Rc::new(Cell::new(None::<(f32, f32)>));
    let selected = create_signal(Vec::<usize>::new());
    {
        let (start, area2) = (start.clone(), area.clone());
        area.on("mousedown", move |event: &Event| {
            start.set(Some((event.mouse_x, event.mouse_y)));
            area2.set_pointer_capture(event.pointer_id).unwrap();
        })
        .unwrap();
    }
    {
        let start = start.clone();
        area.on("mousemove", move |event: &Event| {
            let Some((sx, sy)) = start.get() else { return };
            let (x0, x1) = (sx.min(event.mouse_x), sx.max(event.mouse_x));
            let (y0, y1) = (sy.min(event.mouse_y), sy.max(event.mouse_y));
            let hits = tiles
                .iter()
                .enumerate()
                .filter(|(_, t)| {
                    let r = t.bounding_rect().unwrap().unwrap();
                    r.x < x1 && r.x + r.width > x0 && r.y < y1 && r.y + r.height > y0
                })
                .map(|(i, _)| i)
                .collect();
            selected.set(hits);
        })
        .unwrap();
    }
    {
        let (start, area2) = (start.clone(), area.clone());
        area.on("mouseup", move |event: &Event| {
            start.set(None);
            area2.release_pointer_capture(event.pointer_id).unwrap();
        })
        .unwrap();
    }
    // From the gap left of tile 1 (row 0) to inside tile 6 (row 1).
    mouse(&mut ui, MouseEventType::Down, 100.0, 5.0, MouseButton::Left);
    mouse(
        &mut ui,
        MouseEventType::Move,
        250.0,
        150.0,
        MouseButton::Left,
    );
    mouse(&mut ui, MouseEventType::Up, 250.0, 150.0, MouseButton::Left);
    let hits = selected.get();
    report(
        "select.rubber-band",
        format!("band from (100,5) to (250,150) over a 4x3 grid of 80px tiles: {hits:?}; app computes intersections from bounding_rect per move"),
    );
    assert_eq!(hits, [1, 2, 5, 6]);
}
