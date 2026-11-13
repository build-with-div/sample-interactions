//! The package's vendored `stacking-order` (`lib/vendor/stacking-order.ts`,
//! forked from stacking-order 2.0.0): which of two elements paints in
//! front, from computed styles.

use div::prelude::*;

/// `compare(a, b)`: 1 when `a` is in front of `b`, else -1 (or 0 when
/// their z-indexes are equal and neither is found under the common
/// ancestor).
pub fn compare(a: &Element, b: &Element) -> i32 {
    let mut ancestors_a = ancestors(a);
    let mut ancestors_b = ancestors(b);
    let mut common = None;
    while let (Some(last_a), Some(last_b)) = (ancestors_a.last(), ancestors_b.last()) {
        if !last_a.is_same_node(last_b).unwrap_or(false) {
            break;
        }
        common = ancestors_a.pop();
        ancestors_b.pop();
    }
    let Some(common) = common else {
        // The package asserts a common ancestor.
        return 0;
    };

    let z_a = z_index(find_stacking_context(&ancestors_a));
    let z_b = z_index(find_stacking_context(&ancestors_b));
    if z_a == z_b {
        let furthest_a = ancestors_a.last();
        let furthest_b = ancestors_b.last();
        // Walk the common ancestor's children from the last one.
        let mut children = Vec::new();
        let mut child = common.first_child().ok().flatten();
        while let Some(current) = child {
            child = current.next_sibling().ok().flatten();
            children.push(current);
        }
        for child in children.iter().rev() {
            if furthest_a.is_some_and(|node| node.is_same_node(child).unwrap_or(false)) {
                return 1;
            }
            if furthest_b.is_some_and(|node| node.is_same_node(child).unwrap_or(false)) {
                return -1;
            }
        }
    }
    (z_a - z_b).signum()
}

/// `get_ancestors`: the node, then its parents up to the root.
fn ancestors(node: &Element) -> Vec<Element> {
    let mut list = vec![node.clone()];
    while let Some(parent) = list.last().and_then(|node| node.parent().ok().flatten()) {
        list.push(parent);
    }
    list
}

/// `find_stacking_context`: the outermost of `nodes` (nearest the common
/// ancestor) that creates a stacking context.
fn find_stacking_context(nodes: &[Element]) -> Option<&Element> {
    nodes
        .iter()
        .rev()
        .find(|node| creates_stacking_context(node))
}

fn is_flex_item(node: &Element) -> bool {
    let parent = node.parent().ok().flatten().unwrap_or_else(|| node.clone());
    parent
        .computed_style()
        .is_ok_and(|style| matches!(style.display, Display::Flex | Display::InlineFlex))
}

/// `creates_stacking_context`, over the properties Div computes. Div has
/// no `mix-blend-mode`, `isolation` or `-webkit-overflow-scrolling`, so
/// none of them can create a stacking context in Div either.
fn creates_stacking_context(node: &Element) -> bool {
    let Ok(style) = node.computed_style() else {
        return false;
    };
    if style.position == Position::Fixed {
        return true;
    }
    if style.z_index.is_some() && (style.position != Position::Static || is_flex_item(node)) {
        return true;
    }
    if style.opacity < 1.0 {
        return true;
    }
    if style.native_transform.is_some() || style.transform != Transform2D::IDENTITY {
        return true;
    }
    if style.filter_blur != 0.0 || style.filter_grayscale != 0.0 {
        return true;
    }
    // `will-change` naming a stacking property; Div computes only
    // `will-change: transform`.
    style.will_change_transform
}

/// `get_z_index`: `Number(zIndex) || 0`.
fn z_index(node: Option<&Element>) -> i32 {
    node.and_then(|node| node.computed_style().ok())
        .and_then(|style| style.z_index)
        .unwrap_or(0)
}
