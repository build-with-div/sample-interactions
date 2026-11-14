//! `Group`, `Panel` and `Separator` on Div: the DOM the package renders,
//! and its document-level pointer and separator keyboard handling
//! (`lib/global/mountAxis.ts`, `lib/global/event-handlers`,
//! `lib/global/dom`).
//!
//! Where Div lacks the platform feature the package uses, the code says so
//! with `div-gap: F<n>`; docs/gaps.md lists each one with the feature that
//! replaces it.

use super::cursor::{self, CursorAxis, InteractionState as CursorState};
use super::layout::{
    adjust_layout_by_delta, calculate_default_layout, js_number, layouts_equal,
    separator_aria_values, validate_axis_layout, ItemConstraints, LayoutError, Trigger,
};
use super::number::{format_layout_number, layout_numbers_equal};
use super::size::{Size, UnitContext};
use super::Orientation;
use div::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

type LayoutCallback = Rc<dyn Fn(&[f64])>;
type Content = Box<dyn FnOnce(&Element)>;

/// The `<Panel>` props this port supports.
pub struct Panel {
    id: String,
    default_size: Option<Size>,
    min_size: Option<Size>,
    max_size: Option<Size>,
    collapsed_size: Option<Size>,
    collapsed_threshold: Option<Size>,
    collapsible: bool,
    disabled: bool,
    content: Option<Content>,
    style: Option<Content>,
}

impl Panel {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            default_size: None,
            min_size: None,
            max_size: None,
            collapsed_size: None,
            collapsed_threshold: None,
            collapsible: false,
            disabled: false,
            content: None,
            style: None,
        }
    }

    pub fn default_size(mut self, size: impl Into<Size>) -> Self {
        self.default_size = Some(size.into());
        self
    }

    pub fn min_size(mut self, size: impl Into<Size>) -> Self {
        self.min_size = Some(size.into());
        self
    }

    pub fn max_size(mut self, size: impl Into<Size>) -> Self {
        self.max_size = Some(size.into());
        self
    }

    pub fn collapsed_size(mut self, size: impl Into<Size>) -> Self {
        self.collapsed_size = Some(size.into());
        self
    }

    pub fn collapsed_threshold(mut self, size: impl Into<Size>) -> Self {
        self.collapsed_threshold = Some(size.into());
        self
    }

    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The panel's `style` and `className`, which the package applies to
    /// the inner element.
    pub fn style(mut self, style: impl FnOnce(&Element) + 'static) -> Self {
        self.style = Some(Box::new(style));
        self
    }

    /// Fills the panel's inner element, as the panel's React children do.
    pub fn content(mut self, content: impl FnOnce(&Element) + 'static) -> Self {
        self.content = Some(Box::new(content));
        self
    }
}

/// The `<Separator>` props this port supports.
pub struct Separator {
    id: String,
    disabled: bool,
    style: Option<Content>,
}

impl Separator {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            disabled: false,
            style: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The separator's `style` and `className`: styles the element. They
    /// apply after the package's defaults and before its required styles
    /// (`flex-grow: 0`, `flex-shrink: 0`), as in the package.
    pub fn style(mut self, style: impl FnOnce(&Element) + 'static) -> Self {
        self.style = Some(Box::new(style));
        self
    }
}

enum Child {
    Panel(Panel),
    Separator(Separator),
}

/// The `<Group>` props this port supports.
pub struct Group {
    id: String,
    orientation: Orientation,
    disabled: bool,
    disable_cursor: bool,
    children: Vec<Child>,
    on_layout_change: Option<LayoutCallback>,
    on_layout_changed: Option<LayoutCallback>,
}

impl Group {
    pub fn new(id: impl Into<String>, orientation: Orientation) -> Self {
        Self {
            id: id.into(),
            orientation,
            disabled: false,
            disable_cursor: false,
            children: Vec::new(),
            on_layout_change: None,
            on_layout_changed: None,
        }
    }

    pub fn panel(mut self, panel: Panel) -> Self {
        self.children.push(Child::Panel(panel));
        self
    }

    pub fn separator(mut self, separator: Separator) -> Self {
        self.children.push(Child::Separator(separator));
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn disable_cursor(mut self, disable_cursor: bool) -> Self {
        self.disable_cursor = disable_cursor;
        self
    }

    /// `onLayoutChange`: every layout change, including each drag move.
    pub fn on_layout_change(mut self, callback: impl Fn(&[f64]) + 'static) -> Self {
        self.on_layout_change = Some(Rc::new(callback));
        self
    }

    /// `onLayoutChanged`: a layout change once no pointer drag is active.
    pub fn on_layout_changed(mut self, callback: impl Fn(&[f64]) + 'static) -> Self {
        self.on_layout_changed = Some(Rc::new(callback));
        self
    }

    /// Renders the group into `parent` and registers it, as the package's
    /// layout effect does on mount.
    pub fn mount(self, document: &Document, parent: &Element) -> Result<GroupHandle, MountError> {
        let element = Element::create(document, "div")?;
        element.set_attribute("data-group", "true")?;
        element.set_attribute("id", &self.id)?;
        element.set_height(LengthValue::percent(100.0))?;
        element.set_width(LengthValue::percent(100.0))?;
        element.set_overflow(Overflow::Hidden)?;
        element.set_display(Display::Flex)?;
        element.set_flex_direction(match self.orientation {
            Orientation::Horizontal => FlexDirection::Row,
            Orientation::Vertical => FlexDirection::Column,
        })?;
        element.set_flex_wrap(FlexWrap::Nowrap)?;
        // div-gap: F16 `touch-action: pan-y` (horizontal) or `pan-x`
        // (vertical) on the group; Div has no `touch-action`.

        let mut items = Vec::new();
        let mut separators = Vec::new();
        let mut order = Vec::new();
        for child in self.children {
            match child {
                Child::Panel(panel) => {
                    let outer = Element::create(document, "div")?;
                    if panel.disabled {
                        outer.set_attribute("data-disabled", "true")?;
                    }
                    outer.set_attribute("data-panel", "true")?;
                    outer.set_attribute("id", &panel.id)?;
                    outer.set_display(Display::Flex)?;
                    outer.set_flex_basis(LengthValue::px(0.0))?;
                    outer.set_flex_shrink(1.0)?;
                    outer.set_overflow(Overflow::Visible)?;
                    // Before the group registers, a panel with a default size
                    // takes it as its flex basis; others grow equally.
                    match panel.default_size {
                        Some(size) => outer.set_flex_basis(size_to_length(size))?,
                        None => outer.set_flex_grow(1.0)?,
                    }
                    let inner = Element::create(document, "div")?;
                    inner.set_max_height(LengthValue::percent(100.0))?;
                    inner.set_max_width(LengthValue::percent(100.0))?;
                    inner.set_flex_grow(1.0)?;
                    inner.set_overflow(Overflow::Auto)?;
                    if let Some(style) = panel.style {
                        with_document(document, || style(&inner));
                    }
                    // div-gap: F16 `touch-action` on the panel content.
                    outer.append_child(&inner)?;
                    element.append_child(&outer)?;
                    order.push(Slot::Panel(items.len()));
                    items.push(Item {
                        id: panel.id,
                        element: outer,
                        content: inner,
                        props: ItemProps {
                            default_size: panel.default_size,
                            min_size: panel.min_size,
                            max_size: panel.max_size,
                            collapsed_size: panel.collapsed_size,
                            collapsed_threshold: panel.collapsed_threshold,
                            collapsible: panel.collapsible,
                            disabled: panel.disabled,
                        },
                        pending_content: RefCell::new(panel.content),
                    });
                }
                Child::Separator(separator) => {
                    let node = Element::create(document, "div")?;
                    if separator.disabled {
                        node.set_attribute("aria-disabled", "true")?;
                    }
                    node.set_attribute(
                        "aria-orientation",
                        match self.orientation {
                            Orientation::Horizontal => "vertical",
                            Orientation::Vertical => "horizontal",
                        },
                    )?;
                    node.set_attribute(
                        "data-separator",
                        if separator.disabled {
                            "disabled"
                        } else {
                            "inactive"
                        },
                    )?;
                    node.set_attribute("id", &separator.id)?;
                    // div-gap: F13 `role="separator"` is set as the package
                    // sets it, but Div's accessibility tree ignores `role`.
                    node.set_attribute("role", "separator")?;
                    node.set_flex_basis(LengthValue::auto())?;
                    if separator.disabled && !self.disable_cursor {
                        node.set_cursor(Cursor::NotAllowed)?;
                    }
                    if let Some(style) = separator.style {
                        with_document(document, || style(&node));
                    }
                    node.set_flex_grow(0.0)?;
                    node.set_flex_shrink(0.0)?;
                    // div-gap: F16 `touch-action: none` on the separator.
                    if !separator.disabled {
                        node.set_attribute("tabindex", "0")?;
                    }
                    element.append_child(&node)?;
                    order.push(Slot::Separator(separators.len()));
                    separators.push(SeparatorEntry {
                        element: node,
                        disabled: separator.disabled,
                        focused: Cell::new(false),
                        drag_state: Cell::new(CursorState::Inactive),
                    });
                }
            }
        }
        parent.append_child(&element)?;

        for item in &items {
            if let Some(content) = item.pending_content.borrow_mut().take() {
                with_document(document, || content(&item.content));
            }
        }

        let runtime = Runtime::for_document(document)?;
        let axis = Rc::new(Axis {
            id: self.id,
            element,
            orientation: self.orientation,
            disabled: self.disabled,
            disable_cursor: self.disable_cursor,
            resize_target_minimum_size: (20.0, 10.0),
            items,
            separators,
            order,
            state: RefCell::new(AxisState::default()),
            on_layout_change: self.on_layout_change,
            on_layout_changed: self.on_layout_changed,
            notified_change: RefCell::new(Vec::new()),
            notified_changed: RefCell::new(Vec::new()),
            runtime: Rc::downgrade(&runtime),
        });
        axis.mount(document)?;
        runtime.axes.borrow_mut().push(axis.clone());
        Ok(GroupHandle { axis })
    }
}

fn size_to_length(size: Size) -> LengthValue {
    match size {
        Size::Px(value) => LengthValue::px(value as f32),
        Size::Percent(value) => LengthValue::percent(value as f32),
        Size::Rem(value) => LengthValue::Rem(value as f32),
        Size::Em(value) => LengthValue::Em(value as f32),
        Size::Vh(value) => LengthValue::ViewportHeight(value as f32),
        Size::Vw(value) => LengthValue::ViewportWidth(value as f32),
    }
}

/// A mounted group.
#[derive(Clone)]
pub struct GroupHandle {
    axis: Rc<Axis>,
}

impl GroupHandle {
    /// The group's `id`.
    pub fn id(&self) -> &str {
        &self.axis.id
    }

    /// The current layout, in percent, in panel order.
    pub fn layout(&self) -> Vec<f64> {
        self.axis.state.borrow().layout.clone()
    }

    pub fn element(&self) -> &Element {
        &self.axis.element
    }

    /// The panel's outer (`data-panel`) element.
    pub fn panel_element(&self, id: &str) -> Option<&Element> {
        self.axis
            .items
            .iter()
            .find(|item| item.id == id)
            .map(|item| &item.element)
    }

    /// The panel's inner element, which holds its content.
    pub fn panel_content(&self, id: &str) -> Option<&Element> {
        self.axis
            .items
            .iter()
            .find(|item| item.id == id)
            .map(|item| &item.content)
    }

    pub fn separator_element(&self, id: &str) -> Option<Element> {
        self.axis
            .separators
            .iter()
            .find(|separator| {
                separator
                    .element
                    .get_attribute("id")
                    .ok()
                    .flatten()
                    .as_deref()
                    == Some(id)
            })
            .map(|separator| separator.element.clone())
    }

    /// The constraints derived at the last measurement, in percent.
    pub fn constraints(&self) -> Vec<ItemConstraints> {
        self.axis.state.borrow().constraints.clone()
    }
}

/// The cursor the package would show for this document now: the value of
/// the `*, *:hover { cursor: … !important }` rule it adopts. `None` when
/// it adopts no rule.
pub fn document_cursor(document: &Document) -> Option<&'static str> {
    Runtime::existing(document).and_then(|runtime| runtime.cursor())
}

struct ItemProps {
    default_size: Option<Size>,
    min_size: Option<Size>,
    max_size: Option<Size>,
    collapsed_size: Option<Size>,
    collapsed_threshold: Option<Size>,
    collapsible: bool,
    disabled: bool,
}

struct Item {
    id: String,
    element: Element,
    content: Element,
    props: ItemProps,
    pending_content: RefCell<Option<Content>>,
}

struct SeparatorEntry {
    element: Element,
    disabled: bool,
    focused: Cell<bool>,
    drag_state: Cell<CursorState>,
}

#[derive(Clone, Copy)]
enum Slot {
    Panel(usize),
    Separator(usize),
}

#[derive(Default)]
struct AxisState {
    axis_size: f64,
    constraints: Vec<ItemConstraints>,
    layout: Vec<f64>,
    requested_layout: Vec<f64>,
    /// For each separator, the panels on either side.
    separator_to_items: Vec<Option<(usize, usize)>>,
    /// The size each collapsible panel had before it last collapsed, with
    /// the axis size then (`expandedItemSizes`).
    expanded_sizes: Vec<Option<(f64, f64)>>,
    deferred: bool,
}

struct Axis {
    id: String,
    element: Element,
    orientation: Orientation,
    disabled: bool,
    disable_cursor: bool,
    /// `resizeTargetMinimumSize`: (coarse, fine).
    resize_target_minimum_size: (f64, f64),
    items: Vec<Item>,
    separators: Vec<SeparatorEntry>,
    order: Vec<Slot>,
    state: RefCell<AxisState>,
    on_layout_change: Option<LayoutCallback>,
    on_layout_changed: Option<LayoutCallback>,
    /// The last layout each callback saw (`useMemoizedLayoutCallbacks`).
    notified_change: RefCell<Vec<f64>>,
    notified_changed: RefCell<Vec<f64>>,
    runtime: Weak<Runtime>,
}

#[derive(Clone)]
struct HitRegion {
    axis: Rc<Axis>,
    axis_size: f64,
    items: (usize, usize),
    separator: Option<usize>,
    rect: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Clone)]
enum Interaction {
    Inactive,
    Hover {
        hit_regions: Vec<HitRegion>,
    },
    Active {
        cursor_flags: u8,
        did_pointer_move: bool,
        hit_regions: Vec<HitRegion>,
        initial_layouts: Vec<Vec<f64>>,
        pointer_down_at: Point,
        pointer_id: u64,
        captured: bool,
    },
}

const EMPTY_RECT: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 0.0,
    height: 0.0,
};

/// `getBoundingClientRect()`; an element without a box measures empty.
fn rect_of(element: &Element) -> Rect {
    element.bounding_rect().ok().flatten().unwrap_or(EMPTY_RECT)
}

fn same(a: &Element, b: &Element) -> bool {
    a.is_same_node(b).unwrap_or(false)
}

impl Axis {
    fn mount(self: &Rc<Self>, document: &Document) -> Result<(), MountError> {
        let axis_size = self.available_size();
        let constraints = self.derive_constraints(axis_size, document);
        let default_layout = calculate_default_layout(&constraints);
        let layout = validate_axis_layout(&default_layout, &constraints)?;
        {
            let mut state = self.state.borrow_mut();
            state.expanded_sizes = vec![None; self.items.len()];
            state.separator_to_items = vec![None; self.separators.len()];
        }
        // Include disabled separators, as the package does, so that the
        // keyboard handler finds their panels.
        for region in self.hit_regions(true) {
            if let Some(separator) = region.separator {
                self.state.borrow_mut().separator_to_items[separator] = Some(region.items);
            }
        }
        let requested = normalize_layout(&default_layout);
        self.update(
            AxisUpdate {
                axis_size,
                constraints,
                layout,
                requested_layout: requested,
                deferred: axis_size == 0.0,
            },
            false,
        );
        // The package notifies once after mounting, as a completed change.
        self.notify();

        for (index, separator) in self.separators.iter().enumerate() {
            let weak = Rc::downgrade(self);
            separator.element.on("keydown", move |event: &Event| {
                if let Some(axis) = weak.upgrade() {
                    axis.on_separator_key_down(index, event);
                }
            })?;
            let weak = Rc::downgrade(self);
            separator.element.on("focus", move |_: &Event| {
                if let Some(axis) = weak.upgrade() {
                    axis.separators[index].focused.set(true);
                    axis.refresh_separator_state(index);
                }
            })?;
            let weak = Rc::downgrade(self);
            separator.element.on("blur", move |_: &Event| {
                if let Some(axis) = weak.upgrade() {
                    axis.separators[index].focused.set(false);
                    axis.refresh_separator_state(index);
                }
            })?;
        }
        Ok(())
    }

    /// `calculateAvailableAxisSize`: the sum of the panels' sizes.
    fn available_size(&self) -> f64 {
        // div-gap: F10 `offsetWidth`/`offsetHeight`. The package uses the
        // exact bounding size when it is within 1px of the rounded offset
        // size, which is always the case without transforms; Div has only
        // the bounding size. `getClientScale` is 1 without zoom or scale.
        self.items
            .iter()
            .map(|item| {
                let rect = rect_of(&item.element);
                f64::from(match self.orientation {
                    Orientation::Horizontal => rect.width,
                    Orientation::Vertical => rect.height,
                })
            })
            .sum()
    }

    fn unit_context(&self, document: &Document) -> UnitContext {
        let font_size = |element: &Element| {
            element
                .computed_style()
                .map(|style| f64::from(style.font_size))
                .unwrap_or(16.0)
        };
        let body = document.body();
        let viewport = rect_of(&body);
        UnitContext {
            root_font_size: font_size(&body),
            font_size: self
                .items
                .first()
                .map(|item| font_size(&item.element))
                .unwrap_or(16.0),
            viewport_width: f64::from(viewport.width),
            viewport_height: f64::from(viewport.height),
        }
    }

    /// `calculateItemConstraints`.
    fn derive_constraints(&self, axis_size: f64, document: &Document) -> Vec<ItemConstraints> {
        if axis_size == 0.0 {
            return self
                .items
                .iter()
                .map(|item| ItemConstraints {
                    collapsible: item.props.collapsible,
                    disabled: item.props.disabled,
                    ..ItemConstraints::default()
                })
                .collect();
        }
        let units = self.unit_context(document);
        let percent = |size: Size| {
            format_layout_number((size.to_pixels(axis_size, &units) / axis_size) * 100.0)
        };
        self.items
            .iter()
            .map(|item| ItemConstraints {
                collapsed_size: item.props.collapsed_size.map(percent).unwrap_or(0.0),
                collapsed_threshold: item.props.collapsed_threshold.map(percent),
                collapsible: item.props.collapsible,
                default_size: item.props.default_size.map(percent),
                disabled: item.props.disabled,
                max_size: item.props.max_size.map(percent).unwrap_or(100.0),
                min_size: item.props.min_size.map(percent).unwrap_or(0.0),
            })
            .collect()
    }

    /// `syncAxisStateToCurrentSize`: re-measure, re-derive the pixel based
    /// constraints and revalidate the requested layout.
    fn sync_to_current_size(&self, document: &Document) {
        // div-gap: F8 `ResizeObserver`. The package observes the group and
        // syncs whenever its size changes; without the observer the port
        // syncs when an interaction starts, as the package's imperative API
        // also does.
        let axis_size = self.available_size();
        if axis_size == 0.0 {
            return;
        }
        let constraints = self.derive_constraints(axis_size, document);
        let (requested, deferred, layout_before, constraints_before, size_before) = {
            let state = self.state.borrow();
            (
                state.requested_layout.clone(),
                state.deferred,
                state.layout.clone(),
                state.constraints.clone(),
                state.axis_size,
            )
        };
        let requested = if deferred {
            normalize_layout(&calculate_default_layout(&constraints))
        } else {
            requested
        };
        let Ok(layout) = validate_axis_layout(&requested, &constraints) else {
            return;
        };
        let changed = deferred
            || !layouts_equal(&layout_before, &layout)
            || constraints_before != constraints
            || size_before != axis_size;
        if changed {
            self.update(
                AxisUpdate {
                    axis_size,
                    constraints,
                    layout,
                    requested_layout: requested,
                    deferred: false,
                },
                true,
            );
        }
    }

    /// `updateMountedAxis`: store the state, record collapses
    /// (`recordAxisLayoutChange`), restyle the panels and separators, and
    /// notify.
    fn update(&self, next: AxisUpdate, notify: bool) {
        let previous_layout = {
            let mut state = self.state.borrow_mut();
            let previous_layout = std::mem::replace(&mut state.layout, next.layout.clone());
            let previous_size = state.axis_size;
            if !previous_layout.is_empty() {
                for (index, item) in next.constraints.iter().enumerate() {
                    if !item.collapsible {
                        continue;
                    }
                    let is_collapsed =
                        layout_numbers_equal(item.collapsed_size, next.layout[index], 0.0);
                    let was_collapsed =
                        layout_numbers_equal(item.collapsed_size, previous_layout[index], 0.0);
                    if is_collapsed && !was_collapsed {
                        state.expanded_sizes[index] = Some((previous_size, previous_layout[index]));
                    }
                }
            }
            state.axis_size = next.axis_size;
            state.constraints = next.constraints;
            state.requested_layout = next.requested_layout;
            state.deferred = next.deferred;
            previous_layout
        };

        // `getPanelStyles`: once registered, each panel grows by its
        // percentage from the base `flex-basis: 0`, replacing the default
        // size basis it rendered with before.
        for (index, item) in self.items.iter().enumerate() {
            let _ = item.element.set_flex_basis(LengthValue::px(0.0));
            let _ = item.element.set_flex_grow(next.layout[index] as f32);
        }
        self.refresh_separator_aria();

        let _ = previous_layout;
        // The group's subscription to its own updates (`Group.tsx`).
        if notify {
            self.notify();
        }
    }

    /// `notifyLayoutChange` through `useMemoizedLayoutCallbacks`: each
    /// callback runs only for a layout it has not seen, and
    /// `onLayoutChanged` only once no drag on this group is active.
    fn notify(&self) {
        let (layout, deferred, unconstrained) = {
            let state = self.state.borrow();
            (
                state.layout.clone(),
                state.deferred,
                state.constraints.is_empty(),
            )
        };
        if deferred || unconstrained {
            return;
        }
        let completed =
            self.runtime
                .upgrade()
                .is_none_or(|runtime| match &*runtime.interaction.borrow() {
                    Interaction::Active { hit_regions, .. } => !hit_regions
                        .iter()
                        .any(|region| std::ptr::eq(Rc::as_ptr(&region.axis), self)),
                    _ => true,
                });
        if !layouts_equal(&self.notified_change.borrow(), &layout) {
            *self.notified_change.borrow_mut() = layout.clone();
            if let Some(callback) = &self.on_layout_change {
                callback(&layout);
            }
        }
        if completed && !layouts_equal(&self.notified_changed.borrow(), &layout) {
            *self.notified_changed.borrow_mut() = layout.clone();
            if let Some(callback) = &self.on_layout_changed {
                callback(&layout);
            }
        }
    }

    fn refresh_separator_aria(&self) {
        let state = self.state.borrow();
        for (index, separator) in self.separators.iter().enumerate() {
            let Some((primary, _)) = state.separator_to_items.get(index).copied().flatten() else {
                continue;
            };
            let element = &separator.element;
            let _ = element.set_attribute("aria-controls", &self.items[primary].id);
            match separator_aria_values(&state.layout, &state.constraints, primary) {
                Ok(values) => {
                    let _ = element.set_attribute("aria-valuemax", &js_number(values.value_max));
                    let _ = element.set_attribute("aria-valuemin", &js_number(values.value_min));
                    let _ = element.set_attribute("aria-valuenow", &js_number(values.value_now));
                }
                Err(_) => {
                    let _ = element.remove_attribute("aria-valuemax");
                    let _ = element.remove_attribute("aria-valuemin");
                    let _ = element.remove_attribute("aria-valuenow");
                }
            }
        }
    }

    /// `data-separator`: disabled, active, focus, hover or inactive.
    fn refresh_separator_state(&self, index: usize) {
        let separator = &self.separators[index];
        let value = if separator.disabled {
            "disabled"
        } else {
            match separator.drag_state.get() {
                CursorState::Active => "active",
                state if separator.focused.get() => {
                    let _ = state;
                    "focus"
                }
                CursorState::Hover => "hover",
                CursorState::Inactive => "inactive",
            }
        };
        let _ = separator.element.set_attribute("data-separator", value);
    }

    /// `calculateHitRegions`.
    fn hit_regions(self: &Rc<Self>, include_disabled: bool) -> Vec<HitRegion> {
        let horizontal = self.orientation == Orientation::Horizontal;
        // `sortByElementOffset`: by offset, then size, then DOM order.
        // div-gap: F10 `offsetLeft`/`offsetTop`; the bounding rect orders
        // siblings the same way.
        let mut children: Vec<(Slot, Rect, usize)> = self
            .order
            .iter()
            .enumerate()
            .map(|(dom_index, slot)| {
                let element = match *slot {
                    Slot::Panel(index) => &self.items[index].element,
                    Slot::Separator(index) => &self.separators[index].element,
                };
                (*slot, rect_of(element), dom_index)
            })
            .collect();
        children.sort_by(|a, b| {
            let (offset_a, size_a, offset_b, size_b) = if horizontal {
                (a.1.x, a.1.width, b.1.x, b.1.width)
            } else {
                (a.1.y, a.1.height, b.1.y, b.1.height)
            };
            offset_a
                .total_cmp(&offset_b)
                .then(size_a.total_cmp(&size_b))
                .then(a.2.cmp(&b.2))
        });

        let enabled: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| !item.props.disabled)
            .map(|(index, _)| index)
            .collect();
        let mut regions = Vec::new();
        if !(include_disabled || enabled.len() > 1) {
            return regions;
        }
        let first_enabled = enabled.first().copied().map_or(-1, |index| index as isize);
        let last_enabled = enabled.last().copied().map_or(-1, |index| index as isize);

        let mut disabled_separator = false;
        let mut previous: Option<(usize, Rect)> = None;
        let mut pending_separators: Vec<usize> = Vec::new();
        let mut current_item_index: isize = -1;
        let mut axis_size: Option<f64> = None;
        // The builder renders only panels and separators, so the package's
        // interleaved static content case does not arise.
        for (slot, rect, _) in children {
            match slot {
                Slot::Panel(item_index) => {
                    current_item_index += 1;
                    if let Some((previous_index, previous_rect)) = previous {
                        let targets: Vec<(Rect, Option<usize>)> = if pending_separators.is_empty() {
                            let gap = if horizontal {
                                Rect {
                                    x: previous_rect.x + previous_rect.width,
                                    y: rect.y,
                                    width: rect.x - (previous_rect.x + previous_rect.width),
                                    height: rect.height,
                                }
                            } else {
                                Rect {
                                    x: rect.x,
                                    y: previous_rect.y + previous_rect.height,
                                    width: rect.width,
                                    height: rect.y - (previous_rect.y + previous_rect.height),
                                }
                            };
                            vec![(gap, None)]
                        } else {
                            pending_separators
                                .iter()
                                .map(|&separator| {
                                    (
                                        rect_of(&self.separators[separator].element),
                                        Some(separator),
                                    )
                                })
                                .collect()
                        };
                        for (target, separator) in targets {
                            let rect = self.expand_hit_target(target);
                            let skip = current_item_index <= first_enabled
                                || current_item_index > last_enabled;
                            if include_disabled || (!disabled_separator && !skip) {
                                let size = *axis_size.get_or_insert_with(|| self.available_size());
                                regions.push(HitRegion {
                                    axis: self.clone(),
                                    axis_size: size,
                                    items: (previous_index, item_index),
                                    separator,
                                    rect,
                                });
                            }
                            disabled_separator = false;
                        }
                    }
                    previous = Some((item_index, rect));
                    pending_separators.clear();
                }
                Slot::Separator(index) => {
                    if self.separators[index].disabled {
                        disabled_separator = true;
                    }
                    pending_separators.push(index);
                }
            }
        }
        regions
    }

    /// `expandHitTarget`, with the fine minimum.
    fn expand_hit_target(&self, rect: Rect) -> Rect {
        // div-gap: F20 `matchMedia("(pointer: coarse)")`, which picks the
        // coarse minimum (20px) on touch screens. Div has no `matchMedia`;
        // the fine minimum (10px) applies.
        let minimum = self.resize_target_minimum_size.1 as f32;
        let mut rect = rect;
        if rect.width < minimum {
            let delta = minimum - rect.width;
            rect = Rect {
                x: rect.x - delta / 2.0,
                width: rect.width + delta,
                ..rect
            };
        }
        if rect.height < minimum {
            let delta = minimum - rect.height;
            rect = Rect {
                y: rect.y - delta / 2.0,
                height: rect.height + delta,
                ..rect
            };
        }
        rect
    }

    /// `isPointNearAxis`.
    fn is_point_near(&self, point: Point) -> bool {
        let margin = self
            .resize_target_minimum_size
            .0
            .max(self.resize_target_minimum_size.1);
        let rect = rect_of(&self.element);
        let (left, top) = (f64::from(rect.x), f64::from(rect.y));
        let (right, bottom) = (left + f64::from(rect.width), top + f64::from(rect.height));
        point.x >= left - margin
            && point.x <= right + margin
            && point.y >= top - margin
            && point.y <= bottom + margin
    }

    /// `adjustLayoutForSeparator`, from the keyboard.
    fn adjust_for_separator(&self, separator: usize, delta: f64) {
        let (items, constraints, previous, axis_size, deferred) = {
            let state = self.state.borrow();
            (
                state.separator_to_items[separator],
                state.constraints.clone(),
                state.layout.clone(),
                state.axis_size,
                state.deferred,
            )
        };
        let Some((first, second)) = items else {
            return;
        };
        let previous = if deferred { Vec::new() } else { previous };
        let Ok(unsafe_layout) = adjust_layout_by_delta(
            delta,
            &previous,
            &constraints,
            [first, second],
            &previous,
            Trigger::Keyboard,
        ) else {
            return;
        };
        let Ok(next) = validate_axis_layout(&unsafe_layout, &constraints) else {
            return;
        };
        if !layouts_equal(&previous, &next) {
            self.update(
                AxisUpdate {
                    axis_size,
                    constraints,
                    layout: next.clone(),
                    requested_layout: next,
                    deferred,
                },
                true,
            );
        }
    }

    /// `onDocumentKeyDown`, attached to each separator.
    fn on_separator_key_down(self: &Rc<Self>, index: usize, event: &Event) {
        if event.default_prevented() || self.disabled || self.separators[index].disabled {
            return;
        }
        if let Some(runtime) = self.runtime.upgrade() {
            self.sync_to_current_size(&runtime.document);
        }
        let vertical = self.orientation == Orientation::Vertical;
        match event.key() {
            Some(Key::ArrowDown) => {
                event.prevent_default();
                if vertical {
                    self.adjust_for_separator(index, 5.0);
                }
            }
            Some(Key::ArrowLeft) => {
                event.prevent_default();
                if !vertical {
                    self.adjust_for_separator(index, -5.0);
                }
            }
            Some(Key::ArrowRight) => {
                event.prevent_default();
                if !vertical {
                    self.adjust_for_separator(index, 5.0);
                }
            }
            Some(Key::ArrowUp) => {
                event.prevent_default();
                if vertical {
                    self.adjust_for_separator(index, -5.0);
                }
            }
            Some(Key::End) => {
                event.prevent_default();
                self.adjust_for_separator(index, 100.0);
            }
            Some(Key::Home) => {
                event.prevent_default();
                self.adjust_for_separator(index, -100.0);
            }
            Some(Key::Enter) => {
                event.prevent_default();
                let (constraints, previous_size, expanded, axis_size) = {
                    let state = self.state.borrow();
                    let Some((primary, _)) = state.separator_to_items[index] else {
                        return;
                    };
                    (
                        state.constraints[primary].clone(),
                        state.layout[primary],
                        state.expanded_sizes[primary],
                        state.axis_size,
                    )
                };
                if constraints.collapsible {
                    let next_size = if constraints.collapsed_size == previous_size {
                        match expanded {
                            // `getExpandedItemSize` without
                            // "preserve-pixel-size", the default behavior.
                            Some((_, size)) => {
                                let _ = axis_size;
                                size
                            }
                            None => constraints.min_size,
                        }
                    } else {
                        constraints.collapsed_size
                    };
                    self.adjust_for_separator(index, next_size - previous_size);
                }
            }
            Some(Key::Function(6)) => {
                event.prevent_default();
                let enabled: Vec<&SeparatorEntry> = self
                    .separators
                    .iter()
                    .filter(|separator| !separator.disabled)
                    .collect();
                let current = &self.separators[index].element;
                let Some(position) = enabled
                    .iter()
                    .position(|separator| same(&separator.element, current))
                else {
                    return;
                };
                let shift = Modifiers(event.modifiers as u32).contains(Modifiers::SHIFT);
                let next = if shift {
                    if position > 0 {
                        position - 1
                    } else {
                        enabled.len() - 1
                    }
                } else if position + 1 < enabled.len() {
                    position + 1
                } else {
                    0
                };
                // div-gap: F12 `focus({ preventScroll: true })`. Div's
                // `focus()` never scrolls, which is what the option asks.
                let _ = enabled[next].element.focus();
            }
            _ => {}
        }
    }
}

struct AxisUpdate {
    axis_size: f64,
    constraints: Vec<ItemConstraints>,
    layout: Vec<f64>,
    requested_layout: Vec<f64>,
    deferred: bool,
}

/// `normalizeLayout`: scale to 100% without constraints.
fn normalize_layout(layout: &[f64]) -> Vec<f64> {
    let constraints = vec![ItemConstraints::default(); layout.len()];
    validate_axis_layout(layout, &constraints).unwrap_or_else(|_| layout.to_vec())
}

/// Why a group could not mount.
#[derive(Debug)]
pub enum MountError {
    Div(Error),
    /// The package would throw this from its layout validation.
    Layout(LayoutError),
}

impl From<Error> for MountError {
    fn from(error: Error) -> Self {
        MountError::Div(error)
    }
}

impl From<LayoutError> for MountError {
    fn from(error: LayoutError) -> Self {
        MountError::Layout(error)
    }
}

/// The package's module state for one document: mounted axes and the
/// interaction state, with the document-level listeners.
struct Runtime {
    document: Document,
    body: Element,
    axes: RefCell<Vec<Rc<Axis>>>,
    interaction: RefCell<Interaction>,
    suppress_click: Rc<Cell<bool>>,
}

thread_local! {
    static RUNTIMES: RefCell<Vec<Rc<Runtime>>> = const { RefCell::new(Vec::new()) };
}

impl Runtime {
    fn existing(document: &Document) -> Option<Rc<Runtime>> {
        let body = document.body();
        RUNTIMES.with(|runtimes| {
            runtimes
                .borrow()
                .iter()
                .find(|runtime| same(&runtime.body, &body))
                .cloned()
        })
    }

    fn for_document(document: &Document) -> Result<Rc<Runtime>, Error> {
        if let Some(runtime) = Self::existing(document) {
            return Ok(runtime);
        }
        let runtime = Rc::new(Runtime {
            document: document.clone(),
            body: document.body(),
            axes: RefCell::new(Vec::new()),
            interaction: RefCell::new(Interaction::Inactive),
            suppress_click: Rc::new(Cell::new(false)),
        });
        runtime.install_listeners()?;
        RUNTIMES.with(|runtimes| runtimes.borrow_mut().push(runtime.clone()));
        Ok(runtime)
    }

    /// The listeners `mountAxis` adds to the document for its first group.
    fn install_listeners(self: &Rc<Self>) -> Result<(), Error> {
        let body = &self.body;
        // div-gap: F1 unified Pointer Events. Div reports the mouse only as
        // `mouse*` events and touch only as `pointer*` events; the package
        // listens to `pointer*`. Until Div sends both, mouse events take
        // the same handlers.
        for (pointer, mouse, capture) in [
            ("pointerdown", "mousedown", true),
            ("pointermove", "mousemove", false),
            ("pointerup", "mouseup", true),
            ("pointerleave", "mouseleave", false),
        ] {
            for event_type in [pointer, mouse] {
                let weak = Rc::downgrade(self);
                let handler = move |event: &Event| {
                    if let Some(runtime) = weak.upgrade() {
                        runtime.on_pointer(event);
                    }
                };
                if capture {
                    body.on_capture(event_type, handler)?;
                } else {
                    body.on(event_type, handler)?;
                }
            }
        }
        for event_type in ["pointercancel", "lostpointercapture"] {
            let weak = Rc::downgrade(self);
            body.on_capture(event_type, move |event: &Event| {
                if let Some(runtime) = weak.upgrade() {
                    runtime.abort_active(Some(event.pointer_id));
                }
            })?;
        }
        let weak = Rc::downgrade(self);
        body.on_capture("contextmenu", move |event: &Event| {
            if event.default_prevented() {
                return;
            }
            if let Some(runtime) = weak.upgrade() {
                runtime.complete_active(event, None, false);
            }
        })?;
        // `suppressNextClick`: a capture listener that eats the click after a
        // drag, until the current task ends.
        let suppress = self.suppress_click.clone();
        body.on_capture("click", move |event: &Event| {
            if suppress.replace(false) {
                event.prevent_default();
                event.stop_propagation();
            }
        })?;
        // div-gap: F19 window `blur` and `pagehide`, which end a drag; Div
        // has no window events on the document.
        // div-gap: F1 `dblclick` (resetting a panel to its default size);
        // the handler needs the imperative API, ported in the next PR.
        Ok(())
    }

    fn on_pointer(self: &Rc<Self>, event: &Event) {
        match event.event_type.as_str() {
            "pointerdown" | "mousedown" => self.on_pointer_down(event),
            "pointermove" | "mousemove" => self.on_pointer_move(event),
            "pointerup" | "mouseup" => self.on_pointer_up(event),
            "pointerleave" | "mouseleave" => self.on_pointer_leave(event),
            _ => {}
        }
    }

    fn is_mouse(event: &Event) -> bool {
        // div-gap: F1 `pointerType`. Div's `mouse*` events come from the
        // mouse and its `pointer*` events from touch.
        event.event_type.starts_with("mouse")
    }

    fn point(event: &Event) -> Point {
        Point {
            x: f64::from(event.mouse_x),
            y: f64::from(event.mouse_y),
        }
    }

    /// `findMatchingHitRegions`.
    fn matching_hit_regions(&self, event: &Event) -> Vec<HitRegion> {
        let point = Self::point(event);
        let axes = self.axes.borrow().clone();
        let mut matches = Vec::new();
        for axis in axes {
            if axis.disabled || !axis.is_point_near(point) {
                continue;
            }
            let regions = axis.hit_regions(false);
            if let Some((region, distance)) = closest_hit_region(axis.orientation, &regions, point)
            {
                if distance.x <= 0.0
                    && distance.y <= 0.0
                    && is_viable_hit_target(&axis.element, region.rect, event.target().as_ref())
                {
                    matches.push(region.clone());
                }
            }
        }
        matches
    }

    /// `onDocumentPointerDown`.
    fn on_pointer_down(self: &Rc<Self>, event: &Event) {
        if event.default_prevented() || (Self::is_mouse(event) && event.mouse_button > 0) {
            return;
        }
        if let Interaction::Active { pointer_id, .. } = &*self.interaction.borrow() {
            let same_pointer = *pointer_id == event.pointer_id;
            if !same_pointer {
                return;
            }
        }
        if matches!(*self.interaction.borrow(), Interaction::Active { .. }) {
            // The same pointer pressed twice: its release was missed.
            self.abort_active(Some(event.pointer_id));
            return;
        }

        let axes = self.axes.borrow().clone();
        for axis in &axes {
            axis.sync_to_current_size(&self.document);
        }
        let hit_regions = self.matching_hit_regions(event);
        if hit_regions.is_empty() {
            return;
        }
        let mut focused = false;
        let mut initial_layouts = Vec::new();
        for region in &hit_regions {
            if let Some(separator) = region.separator {
                if !focused {
                    focused = true;
                    // div-gap: F12 `focus({ focusVisible: false,
                    // preventScroll: true })`. Div's `focus()` does not
                    // scroll; it has no `focusVisible` option.
                    let _ = region.axis.separators[separator].element.focus();
                }
            }
            initial_layouts.push(region.axis.state.borrow().layout.clone());
        }
        self.set_interaction(Interaction::Active {
            cursor_flags: 0,
            did_pointer_move: false,
            hit_regions,
            initial_layouts,
            pointer_down_at: Self::point(event),
            pointer_id: event.pointer_id,
            captured: false,
        });
        event.prevent_default();
    }

    /// `onDocumentPointerMove`.
    fn on_pointer_move(self: &Rc<Self>, event: &Event) {
        if event.default_prevented() {
            return;
        }
        let interaction = self.interaction.borrow().clone();
        match interaction {
            Interaction::Active {
                pointer_id,
                hit_regions,
                captured,
                ..
            } => {
                if pointer_id != event.pointer_id {
                    return;
                }
                // div-gap: F1 `buttons`. The package ends the drag when a
                // move reports no buttons (a release it missed); Div's
                // events carry no `buttons`.
                if !captured {
                    // div-gap: F1 `hasPointerCapture`. The package captures on
                    // the first move unless the separator already holds the
                    // pointer; Div cannot report capture, so the port
                    // captures once per drag.
                    for region in &hit_regions {
                        if let Some(separator) = region.separator {
                            let element = &region.axis.separators[separator].element;
                            if element.is_connected().unwrap_or(false) {
                                let _ = element.set_pointer_capture(event.pointer_id);
                            }
                        }
                    }
                    if let Interaction::Active { captured, .. } =
                        &mut *self.interaction.borrow_mut()
                    {
                        *captured = true;
                    }
                }
                self.update_active_hit_regions(event, false, true);
            }
            other => {
                let hit_regions = self.matching_hit_regions(event);
                if hit_regions.is_empty() {
                    if !matches!(other, Interaction::Inactive) {
                        self.set_interaction(Interaction::Inactive);
                    }
                } else {
                    let unchanged = match &other {
                        Interaction::Hover {
                            hit_regions: previous,
                        } => hit_regions_equal(previous, &hit_regions),
                        _ => false,
                    };
                    if !unchanged {
                        self.set_interaction(Interaction::Hover { hit_regions });
                    }
                }
            }
        }
    }

    /// `onDocumentPointerUp`.
    fn on_pointer_up(self: &Rc<Self>, event: &Event) {
        if event.default_prevented() || (Self::is_mouse(event) && event.mouse_button > 0) {
            return;
        }
        // The package prevents the `pointerup` that completes a drag, which
        // in Chromium does not cancel the `click`.
        // div-gap: F1 a cancelled `mouseup` or `pointerup` suppresses the
        // `click` in Div (`divui/src/document.rs:1108`). The mouse arrives
        // as `mouseup` here, so the port does not cancel it.
        let prevent = !Self::is_mouse(event);
        self.complete_active(event, Some(event.pointer_id), prevent);
    }

    /// `onDocumentPointerLeave`.
    fn on_pointer_leave(self: &Rc<Self>, event: &Event) {
        let active = matches!(
            &*self.interaction.borrow(),
            Interaction::Active { pointer_id, .. } if *pointer_id == event.pointer_id
        );
        if active {
            self.update_active_hit_regions(event, false, false);
        }
    }

    /// `updateActiveHitRegions`. With `from_pointer_down` false (a leave),
    /// the delta is a full move to the edge the pointer left by.
    fn update_active_hit_regions(&self, event: &Event, _commit: bool, from_pointer_down: bool) {
        let Interaction::Active {
            hit_regions,
            initial_layouts,
            pointer_down_at,
            cursor_flags: previous_flags,
            ..
        } = self.interaction.borrow().clone()
        else {
            return;
        };
        let point = Self::point(event);
        let mut next_flags = 0u8;
        for (region, initial) in hit_regions.iter().zip(&initial_layouts) {
            let axis = &region.axis;
            let horizontal = axis.orientation == Orientation::Horizontal;
            let delta = if from_pointer_down {
                if horizontal {
                    ((point.x - pointer_down_at.x) / region.axis_size) * 100.0
                } else {
                    ((point.y - pointer_down_at.y) / region.axis_size) * 100.0
                }
            } else {
                let coordinate = if horizontal { point.x } else { point.y };
                if coordinate < 0.0 {
                    -100.0
                } else {
                    100.0
                }
            };
            let (constraints, mounted, axis_size, deferred) = {
                let state = axis.state.borrow();
                (
                    state.constraints.clone(),
                    state.layout.clone(),
                    state.axis_size,
                    state.deferred,
                )
            };
            let Ok(next) = adjust_layout_by_delta(
                delta,
                initial,
                &constraints,
                [region.items.0, region.items.1],
                &mounted,
                Trigger::MouseOrTouch,
            ) else {
                continue;
            };
            if layouts_equal(&next, &mounted) {
                if delta != 0.0 && !axis.disable_cursor {
                    next_flags |= match (horizontal, delta < 0.0) {
                        (true, true) => cursor::CURSOR_FLAG_HORIZONTAL_MIN,
                        (true, false) => cursor::CURSOR_FLAG_HORIZONTAL_MAX,
                        (false, true) => cursor::CURSOR_FLAG_VERTICAL_MIN,
                        (false, false) => cursor::CURSOR_FLAG_VERTICAL_MAX,
                    };
                }
            } else {
                axis.update(
                    AxisUpdate {
                        axis_size,
                        constraints,
                        layout: next.clone(),
                        requested_layout: next,
                        deferred,
                    },
                    true,
                );
            }
        }
        // div-gap: F1 `movementX`/`movementY`. The package keeps the
        // previous flags on an axis with no movement; without movement
        // values every event counts as movement.
        let _ = previous_flags;
        let did_pointer_move = point != pointer_down_at;
        if let Interaction::Active {
            cursor_flags,
            did_pointer_move: moved,
            ..
        } = &mut *self.interaction.borrow_mut()
        {
            *cursor_flags = next_flags;
            *moved = did_pointer_move;
        }
        self.refresh_cursor();
    }

    /// `completeActivePointerResize`: commit at the event's position.
    fn complete_active(self: &Rc<Self>, event: &Event, pointer: Option<u64>, prevent: bool) {
        let (active, regions) = match &*self.interaction.borrow() {
            Interaction::Active {
                pointer_id,
                hit_regions,
                ..
            } => (
                pointer.is_none_or(|id| id == *pointer_id),
                hit_regions.clone(),
            ),
            _ => (false, Vec::new()),
        };
        if !active {
            return;
        }
        // With the package's default `resizePreviewMode` ("panel"), commit
        // does not re-run the layout; it was applied on each move.
        if prevent && !regions.is_empty() {
            event.prevent_default();
        }
        // div-gap: F1 implicit pointer capture release after `pointerup`.
        for region in &regions {
            if let Some(separator) = region.separator {
                let _ = region.axis.separators[separator]
                    .element
                    .release_pointer_capture(event.pointer_id);
            }
        }
        self.finish_active();
    }

    /// `abortActivePointerResize`.
    fn abort_active(self: &Rc<Self>, pointer: Option<u64>) {
        let active = matches!(
            &*self.interaction.borrow(),
            Interaction::Active { pointer_id, .. } if pointer.is_none_or(|id| id == *pointer_id)
        );
        if active {
            self.finish_active();
        }
    }

    /// `finishActivePointerResize`: back to inactive, then tell each axis
    /// the gesture ended (which fires `onLayoutChanged`).
    fn finish_active(self: &Rc<Self>) {
        let previous = self.interaction.borrow().clone();
        self.set_interaction(Interaction::Inactive);
        if let Interaction::Active {
            hit_regions,
            initial_layouts,
            ..
        } = previous
        {
            let _ = initial_layouts;
            for region in &hit_regions {
                region.axis.notify();
            }
        }
    }

    /// `updateInteractionState`: blur and click suppression after a drag,
    /// then the separators' `data-separator` and the cursor.
    fn set_interaction(&self, next: Interaction) {
        let previous = std::mem::replace(&mut *self.interaction.borrow_mut(), next);
        let next_active = matches!(*self.interaction.borrow(), Interaction::Active { .. });
        if let Interaction::Active {
            did_pointer_move: true,
            hit_regions,
            ..
        } = &previous
        {
            if !next_active {
                for region in hit_regions {
                    if let Some(separator) = region.separator {
                        let element = &region.axis.separators[separator].element;
                        if element.has_focus().unwrap_or(false) {
                            let _ = element.blur();
                        }
                    }
                }
                self.suppress_click.set(true);
                let suppress = self.suppress_click.clone();
                div::task::spawn_local(async move {
                    suppress.set(false);
                })
                .detach();
            }
        }
        self.refresh_separators();
        self.refresh_cursor();
    }

    fn refresh_separators(&self) {
        let (state, regions) = match &*self.interaction.borrow() {
            Interaction::Inactive => (CursorState::Inactive, Vec::new()),
            Interaction::Hover { hit_regions } => (CursorState::Hover, hit_regions.clone()),
            Interaction::Active { hit_regions, .. } => (CursorState::Active, hit_regions.clone()),
        };
        for axis in self.axes.borrow().iter() {
            for (index, separator) in axis.separators.iter().enumerate() {
                let hit = regions.iter().any(|region| {
                    Rc::ptr_eq(&region.axis, axis) && region.separator == Some(index)
                });
                separator
                    .drag_state
                    .set(if hit { state } else { CursorState::Inactive });
                axis.refresh_separator_state(index);
            }
        }
    }

    fn cursor(&self) -> Option<&'static str> {
        let (state, flags, regions) = match &*self.interaction.borrow() {
            Interaction::Inactive => (CursorState::Inactive, 0, Vec::new()),
            Interaction::Hover { hit_regions } => (CursorState::Hover, 0, hit_regions.clone()),
            Interaction::Active {
                hit_regions,
                cursor_flags,
                ..
            } => (CursorState::Active, *cursor_flags, hit_regions.clone()),
        };
        let axes: Vec<CursorAxis> = regions
            .iter()
            .map(|region| CursorAxis {
                orientation: region.axis.orientation,
                disable_cursor: region.axis.disable_cursor,
            })
            .collect();
        cursor::cursor_style(flags, &axes, state)
    }

    fn refresh_cursor(&self) {
        // div-gap: F17 cursor keywords (`ew-resize`, `ns-resize`, the
        // one-sided `*-resize` values) and a runtime stylesheet
        // (`adoptedStyleSheets`, decision 2: later). The package adopts
        // `*, *:hover { cursor: <value> !important }`; `document_cursor`
        // reports the value until Div can apply it.
        let _ = self.cursor();
    }
}

#[derive(Clone, Copy)]
struct Distance {
    x: f64,
    y: f64,
}

/// `getDistanceBetweenPointAndRect`.
fn distance_to(point: Point, rect: Rect) -> Distance {
    let (left, top) = (f64::from(rect.x), f64::from(rect.y));
    let (right, bottom) = (left + f64::from(rect.width), top + f64::from(rect.height));
    Distance {
        x: if point.x >= left && point.x <= right {
            0.0
        } else {
            (point.x - left).abs().min((point.x - right).abs())
        },
        y: if point.y >= top && point.y <= bottom {
            0.0
        } else {
            (point.y - top).abs().min((point.y - bottom).abs())
        },
    }
}

/// `findClosestHitRegion`.
fn closest_hit_region(
    orientation: Orientation,
    regions: &[HitRegion],
    point: Point,
) -> Option<(&HitRegion, Distance)> {
    let mut closest: Option<(&HitRegion, Distance)> = None;
    let mut minimum = Distance {
        x: f64::INFINITY,
        y: f64::INFINITY,
    };
    for region in regions {
        let data = distance_to(point, region.rect);
        let better = match orientation {
            Orientation::Horizontal => {
                data.x < minimum.x || (data.x == minimum.x && data.y <= minimum.y)
            }
            Orientation::Vertical => {
                data.y < minimum.y || (data.y == minimum.y && data.x <= minimum.x)
            }
        };
        if better {
            closest = Some((region, data));
            minimum = data;
        }
    }
    closest
}

/// `isViableHitTarget`: is something else above the separator at the
/// pointer?
fn is_viable_hit_target(
    axis_element: &Element,
    hit_region: Rect,
    target: Option<&Element>,
) -> bool {
    // div-gap: F4 `<dialog>` and `showModal()`. The package first rejects
    // targets inside a modal dialog that does not contain the group; Div has
    // no dialog element.
    let Some(target) = target else {
        return true;
    };
    if target.contains(axis_element).unwrap_or(false)
        || axis_element.contains(target).unwrap_or(false)
    {
        return true;
    }
    if !target.is_connected().unwrap_or(false) || !axis_element.is_connected().unwrap_or(false) {
        return true;
    }
    if super::stacking::compare(target, axis_element) > 0 {
        let mut current = Some(target.clone());
        while let Some(element) = current {
            if element.contains(axis_element).unwrap_or(false) {
                return true;
            }
            if rects_intersect(rect_of(&element), hit_region) {
                return false;
            }
            current = element.parent().ok().flatten();
        }
    }
    true
}

/// `doRectsIntersect`.
fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

/// `hitRegionsEqual`: same axes, separators and items; rects and axis
/// sizes are ignored.
fn hit_regions_equal(a: &[HitRegion], b: &[HitRegion]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            Rc::ptr_eq(&a.axis, &b.axis) && a.items == b.items && a.separator == b.separator
        })
}
