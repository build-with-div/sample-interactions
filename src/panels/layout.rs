//! The layout algorithm of react-resizable-panels 4.22.1
//! (`lib/global/utils`), ported line for line. All sizes are percentages
//! of the group (0 to 100); other units are converted before they reach
//! this module.
//!
//! The package keys layouts by panel id. Every function here walks the
//! constraints in panel order, as the package does, so a layout is a
//! `Vec<f64>` in that order. `tests/panels_oracle.rs` checks each function
//! against the package's own output.

use super::number::{compare_layout_numbers, format_layout_number, layout_numbers_equal};
use std::cmp::Ordering;

/// One panel's constraints, in percent (`ResizeItemConstraints`).
#[derive(Debug, Clone, PartialEq)]
pub struct ItemConstraints {
    pub collapsed_size: f64,
    pub collapsed_threshold: Option<f64>,
    pub collapsible: bool,
    pub default_size: Option<f64>,
    pub disabled: bool,
    pub max_size: f64,
    pub min_size: f64,
}

impl Default for ItemConstraints {
    fn default() -> Self {
        Self {
            collapsed_size: 0.0,
            collapsed_threshold: None,
            collapsible: false,
            default_size: None,
            disabled: false,
            max_size: 100.0,
            min_size: 0.0,
        }
    }
}

/// What started a resize; the package's `trigger`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    ImperativeApi,
    Keyboard,
    MouseOrTouch,
}

/// The package throws (through `assert`) on malformed input. The port
/// returns the same message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutError(pub String);

fn size_at(layout: &[f64], index: usize, what: &str) -> Result<f64, LayoutError> {
    layout
        .get(index)
        .copied()
        .ok_or_else(|| LayoutError(format!("{what} {index}")))
}

/// `validateItemSize`.
pub fn validate_item_size(
    override_disabled_items: bool,
    constraints: &ItemConstraints,
    prev_size: f64,
    size: f64,
) -> f64 {
    let ItemConstraints {
        collapsed_size,
        collapsed_threshold,
        collapsible,
        disabled,
        max_size,
        min_size,
        ..
    } = *constraints;

    if disabled && !override_disabled_items {
        return prev_size;
    }

    let mut size = size;
    if compare_layout_numbers(size, min_size) == Ordering::Less {
        if collapsible {
            let threshold = collapsed_threshold.unwrap_or((min_size - collapsed_size) / 2.0);
            let was_collapsed =
                compare_layout_numbers(prev_size, collapsed_size) != Ordering::Greater;
            let boundary = if was_collapsed {
                collapsed_size + threshold
            } else {
                min_size - threshold
            };
            let comparison = compare_layout_numbers(size, boundary);

            if compare_layout_numbers(size, collapsed_size) != Ordering::Greater
                || comparison == Ordering::Less
                || (collapsed_threshold.is_some() && was_collapsed && comparison == Ordering::Equal)
            {
                size = collapsed_size;
            } else {
                size = min_size;
            }
        } else {
            size = min_size;
        }
    }

    // `Math.min(maxSize, size)`.
    size = js_min(max_size, size);
    format_layout_number(size)
}

/// `Math.min(a, b)` for two numbers: NaN wins, and -0 is less than +0.
fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == b {
        if a.is_sign_negative() {
            a
        } else {
            b
        }
    } else {
        a.min(b)
    }
}

/// `adjustLayoutByDelta`. `initial_layout` is the layout when the gesture
/// started; `prev_layout` is the layout now, returned unchanged when the
/// delta cannot be applied.
pub fn adjust_layout_by_delta(
    delta: f64,
    initial_layout: &[f64],
    constraints: &[ItemConstraints],
    pivot_indices: [usize; 2],
    prev_layout: &[f64],
    trigger: Trigger,
) -> Result<Vec<f64>, LayoutError> {
    let mut delta = delta;
    if layout_numbers_equal(delta, 0.0, 0.0) {
        return Ok(initial_layout.to_vec());
    }

    let override_disabled_items = trigger == Trigger::ImperativeApi;
    let count = constraints.len();
    let initial: Vec<f64> = (0..count)
        .map(|index| {
            size_at(
                initial_layout,
                index,
                "Previous layout not found for panel index",
            )
        })
        .collect::<Result<_, _>>()?;
    let prev: Vec<f64> = (0..count)
        .map(|index| {
            size_at(
                prev_layout,
                index,
                "Previous layout not found for panel index",
            )
        })
        .collect::<Result<_, _>>()?;
    let mut next = initial.clone();

    let [first_pivot, second_pivot] = pivot_indices;
    let constraint_at = |index: usize| -> Result<&ItemConstraints, LayoutError> {
        constraints
            .get(index)
            .ok_or_else(|| LayoutError(format!("Panel constraints not found for index {index}")))
    };
    let initial_at =
        |index: usize| size_at(&initial, index, "Previous layout not found for panel index");

    let mut delta_applied = 0.0;

    match trigger {
        Trigger::Keyboard => {
            {
                // Expand a collapsed panel.
                let index = if delta < 0.0 {
                    second_pivot
                } else {
                    first_pivot
                };
                let item = constraint_at(index)?;
                if item.collapsible {
                    let prev_size = initial_at(index)?;
                    if layout_numbers_equal(prev_size, item.collapsed_size, 0.0) {
                        let local_delta = item.min_size - prev_size;
                        if compare_layout_numbers(local_delta, delta.abs()) == Ordering::Greater {
                            delta = if delta < 0.0 {
                                0.0 - local_delta
                            } else {
                                local_delta
                            };
                        }
                    }
                }
            }
            {
                // Collapse a panel at its minimum size.
                let index = if delta < 0.0 {
                    first_pivot
                } else {
                    second_pivot
                };
                let item = constraint_at(index)?;
                if item.collapsible {
                    let prev_size = initial_at(index)?;
                    if layout_numbers_equal(prev_size, item.min_size, 0.0) {
                        let local_delta = prev_size - item.collapsed_size;
                        if compare_layout_numbers(local_delta, delta.abs()) == Ordering::Greater {
                            delta = if delta < 0.0 {
                                0.0 - local_delta
                            } else {
                                local_delta
                            };
                        }
                    }
                }
            }
        }
        Trigger::ImperativeApi | Trigger::MouseOrTouch => {
            // Dragging a collapsed panel past the threshold expands it.
            let index = if delta < 0.0 {
                second_pivot
            } else {
                first_pivot
            };
            let item = constraint_at(index)?;
            // `initialLayout[index]` may be undefined in the package; every
            // comparison with undefined is false, which NaN reproduces.
            let prev_size = initial.get(index).copied().unwrap_or(f64::NAN);
            if item.collapsible
                && compare_layout_numbers(prev_size, item.min_size) == Ordering::Less
            {
                let gap_size = item.min_size - item.collapsed_size;
                let threshold = item.collapsed_threshold.unwrap_or(gap_size / 2.0);
                let next_size = prev_size + delta.abs();
                if compare_layout_numbers(next_size, item.min_size) == Ordering::Less {
                    let comparison = compare_layout_numbers(delta.abs(), threshold);
                    let expand_at_boundary = item.collapsed_threshold.is_none() && delta < 0.0;
                    if comparison == Ordering::Greater
                        || (comparison == Ordering::Equal && expand_at_boundary)
                    {
                        delta = if delta < 0.0 { -gap_size } else { gap_size };
                    } else {
                        delta = 0.0;
                    }
                }
            }
        }
    }

    {
        // The most the panels on the far side of the pivot can give.
        let increment: isize = if delta < 0.0 { 1 } else { -1 };
        let mut index = (if delta < 0.0 {
            second_pivot
        } else {
            first_pivot
        }) as isize;
        let mut max_available_delta = 0.0;
        loop {
            let prev_size = initial_at(index as usize)?;
            let max_safe_size = validate_item_size(
                override_disabled_items,
                constraint_at(index as usize)?,
                prev_size,
                100.0,
            );
            max_available_delta += max_safe_size - prev_size;
            index += increment;
            if index < 0 || index as usize >= count {
                break;
            }
        }
        let min_abs_delta = delta.abs().min(max_available_delta.abs());
        delta = if delta < 0.0 {
            0.0 - min_abs_delta
        } else {
            min_abs_delta
        };
    }

    {
        // Take the delta from the panels on the near side, in order.
        let mut index = (if delta < 0.0 {
            first_pivot
        } else {
            second_pivot
        }) as isize;
        while index >= 0 && (index as usize) < count {
            let i = index as usize;
            let delta_remaining = delta.abs() - f64::abs(delta_applied);
            let prev_size = initial_at(i)?;
            let unsafe_size = prev_size - delta_remaining;
            let safe_size = validate_item_size(
                override_disabled_items,
                constraint_at(i)?,
                prev_size,
                unsafe_size,
            );
            if !layout_numbers_equal(prev_size, safe_size, 0.0) {
                delta_applied += prev_size - safe_size;
                next[i] = safe_size;
                if compare_layout_numbers(delta_applied, delta.abs()) != Ordering::Less {
                    break;
                }
            }
            if delta < 0.0 {
                index -= 1;
            } else {
                index += 1;
            }
        }
    }

    // `isArrayEqual(prevLayout, nextLayout)` uses `!=`.
    if prev.len() == next.len() && prev.iter().zip(&next).all(|(a, b)| a == b) {
        return Ok(prev_layout.to_vec());
    }

    {
        // Give what was taken to the pivot panel on the other side.
        let pivot = if delta < 0.0 {
            second_pivot
        } else {
            first_pivot
        };
        let prev_size = initial_at(pivot)?;
        let unsafe_size = prev_size + delta_applied;
        let safe_size = validate_item_size(
            override_disabled_items,
            constraint_at(pivot)?,
            prev_size,
            unsafe_size,
        );
        next[pivot] = safe_size;

        // Expanding or contracting one panel changed another's collapsed
        // state: spread the remainder.
        if !layout_numbers_equal(safe_size, unsafe_size, 0.0) {
            let mut delta_remaining = unsafe_size - safe_size;
            let mut index = pivot as isize;
            while index >= 0 && (index as usize) < count {
                let i = index as usize;
                let prev_size = next[i];
                let unsafe_size = prev_size + delta_remaining;
                let safe_size = validate_item_size(
                    override_disabled_items,
                    constraint_at(i)?,
                    prev_size,
                    unsafe_size,
                );
                if !layout_numbers_equal(prev_size, safe_size, 0.0) {
                    delta_remaining -= safe_size - prev_size;
                    next[i] = safe_size;
                }
                if layout_numbers_equal(delta_remaining, 0.0, 0.0) {
                    break;
                }
                if delta > 0.0 {
                    index -= 1;
                } else {
                    index += 1;
                }
            }
        }
    }

    // `nextLayout.reduce((total, size) => size + total, 0)`.
    let total = next.iter().fold(0.0, |total, size| size + total);
    if !layout_numbers_equal(total, 100.0, 0.1) {
        return Ok(prev_layout.to_vec());
    }
    Ok(next)
}

/// `validateAxisLayout`: scale a layout to 100% and fit each panel to its
/// constraints.
pub fn validate_axis_layout(
    layout: &[f64],
    constraints: &[ItemConstraints],
) -> Result<Vec<f64>, LayoutError> {
    if layout.len() != constraints.len() {
        let sizes: Vec<String> = layout
            .iter()
            .map(|size| format!("{}%", js_number(*size)))
            .collect();
        return Err(LayoutError(format!(
            "Invalid {} panel layout: {}",
            constraints.len(),
            sizes.join(", ")
        )));
    }
    let prev = layout.to_vec();
    let mut next = prev.clone();
    // `reduce((accumulated, current) => accumulated + current, 0)`.
    let total = next
        .iter()
        .fold(0.0, |accumulated, current| accumulated + current);
    if !layout_numbers_equal(total, 100.0, 0.0) && !next.is_empty() {
        for size in next.iter_mut() {
            // `(100 / total) * size`; IEEE multiplication commutes.
            *size *= 100.0 / total;
        }
    }

    let mut remaining_size = 0.0;
    for (index, item) in constraints.iter().enumerate() {
        let unsafe_size = next[index];
        let safe_size = validate_item_size(true, item, prev[index], unsafe_size);
        if unsafe_size != safe_size {
            remaining_size += unsafe_size - safe_size;
            next[index] = safe_size;
        }
    }

    if !layout_numbers_equal(remaining_size, 0.0, 0.0) {
        for (index, item) in constraints.iter().enumerate() {
            let prev_size = next[index];
            let unsafe_size = prev_size + remaining_size;
            let safe_size = validate_item_size(true, item, prev_size, unsafe_size);
            if prev_size != safe_size {
                remaining_size -= safe_size - prev_size;
                next[index] = safe_size;
                if layout_numbers_equal(remaining_size, 0.0, 0.0) {
                    break;
                }
            }
        }
    }
    Ok(next)
}

/// `calculateDefaultLayout`: explicit default sizes, with the rest shared
/// equally.
pub fn calculate_default_layout(constraints: &[ItemConstraints]) -> Vec<f64> {
    let mut explicit_count = 0usize;
    let mut total = 0.0;
    let mut layout: Vec<Option<f64>> = Vec::with_capacity(constraints.len());
    for item in constraints {
        match item.default_size {
            Some(default_size) => {
                explicit_count += 1;
                let size = format_layout_number(default_size);
                total += size;
                layout.push(Some(size));
            }
            None => layout.push(None),
        }
    }
    let remaining = constraints.len() - explicit_count;
    let shared = if remaining != 0 {
        Some(format_layout_number((100.0 - total) / remaining as f64))
    } else {
        None
    };
    layout
        .into_iter()
        .map(|size| size.or(shared).expect("every size assigned"))
        .collect()
}

/// `layoutsEqual`.
pub fn layouts_equal(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(a, b)| compare_layout_numbers(*a, *b) == Ordering::Equal)
}

/// The separator's `aria-valuemin`, `aria-valuemax` and `aria-valuenow`
/// (`calculateSeparatorAriaValues`), for the separator after the panel at
/// `item_index`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeparatorAriaValues {
    pub value_min: f64,
    pub value_max: f64,
    pub value_now: f64,
}

pub fn separator_aria_values(
    layout: &[f64],
    constraints: &[ItemConstraints],
    item_index: usize,
) -> Result<SeparatorAriaValues, LayoutError> {
    let item = constraints.get(item_index).ok_or_else(|| {
        LayoutError(format!(
            "Panel constraints not found for index {item_index}"
        ))
    })?;
    let item_size = size_at(layout, item_index, "No layout data found for index")?;
    let min_size = if item.collapsible {
        item.collapsed_size
    } else {
        item.min_size
    };
    let pivots = [item_index, item_index + 1];
    // The package passes no trigger here, which takes the pointer path and
    // does not override disabled panels: the same as `MouseOrTouch`.
    let extreme = |target: f64| -> Result<f64, LayoutError> {
        let adjusted = adjust_layout_by_delta(
            target - item_size,
            layout,
            constraints,
            pivots,
            layout,
            Trigger::MouseOrTouch,
        )?;
        Ok(validate_axis_layout(&adjusted, constraints)?[item_index])
    };
    Ok(SeparatorAriaValues {
        value_min: extreme(min_size)?,
        value_max: extreme(item.max_size)?,
        value_now: item_size,
    })
}

/// Formats a number as JavaScript's `String(number)` does, for messages
/// and attribute values.
pub fn js_number(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    let magnitude = value.abs();
    if (1e-6..1e21).contains(&magnitude) {
        // Rust's Display gives the shortest round-trip digits, as
        // JavaScript does, in plain notation over this range.
        format!("{value}")
    } else {
        // JavaScript uses exponent notation outside it: 1e-7, 1.5e+21.
        let text = format!("{value:e}");
        match text.split_once('e') {
            Some((mantissa, exponent)) if !exponent.starts_with('-') => {
                format!("{mantissa}e+{exponent}")
            }
            _ => text,
        }
    }
}
