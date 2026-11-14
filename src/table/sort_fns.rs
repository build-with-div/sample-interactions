//! table-core's built-in sort functions (`features/row-sorting/sortFns.ts`).

use std::cmp::Ordering;

use super::value::{compare_utf16, greater_than, less, strict_equals, Value};
use crate::panels::layout::js_number;

/// The sort functions table-core ships, by their registry names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinSortFn {
    Alphanumeric,
    AlphanumericCaseSensitive,
    Text,
    TextCaseSensitive,
    Datetime,
    Basic,
}

impl BuiltinSortFn {
    /// Every built-in, in the order of table-core's `sortFns` export.
    pub const ALL: [BuiltinSortFn; 6] = [
        BuiltinSortFn::Alphanumeric,
        BuiltinSortFn::AlphanumericCaseSensitive,
        BuiltinSortFn::Basic,
        BuiltinSortFn::Datetime,
        BuiltinSortFn::Text,
        BuiltinSortFn::TextCaseSensitive,
    ];

    /// The registry name (`sortFns.alphanumeric`, …).
    pub fn name(self) -> &'static str {
        match self {
            BuiltinSortFn::Alphanumeric => "alphanumeric",
            BuiltinSortFn::AlphanumericCaseSensitive => "alphanumericCaseSensitive",
            BuiltinSortFn::Text => "text",
            BuiltinSortFn::TextCaseSensitive => "textCaseSensitive",
            BuiltinSortFn::Datetime => "datetime",
            BuiltinSortFn::Basic => "basic",
        }
    }

    pub fn from_name(name: &str) -> Option<BuiltinSortFn> {
        BuiltinSortFn::ALL.into_iter().find(|f| f.name() == name)
    }

    /// The function applied to two cell values: -1, 0 or 1.
    pub fn compare(self, a: &Value, b: &Value) -> f64 {
        match self {
            BuiltinSortFn::Alphanumeric => compare_alphanumeric(&lower(a), &lower(b)),
            BuiltinSortFn::AlphanumericCaseSensitive => {
                compare_alphanumeric(&sort_string(a), &sort_string(b))
            }
            BuiltinSortFn::Text => compare_strings(&lower(a), &lower(b)),
            BuiltinSortFn::TextCaseSensitive => compare_strings(&sort_string(a), &sort_string(b)),
            BuiltinSortFn::Datetime => {
                let a = date_sort_value(a);
                let b = date_sort_value(b);
                if greater_than(&a, &b) {
                    1.0
                } else if less(&a, &b) {
                    -1.0
                } else {
                    0.0
                }
            }
            BuiltinSortFn::Basic => compare_basic(a, b),
        }
    }
}

/// `compareBasic`: `a === b ? 0 : a > b ? 1 : -1`.
fn compare_basic(a: &Value, b: &Value) -> f64 {
    if strict_equals(a, b) {
        0.0
    } else if greater_than(a, b) {
        1.0
    } else {
        -1.0
    }
}

/// `compareBasic` on two strings.
fn compare_strings(a: &str, b: &str) -> f64 {
    match compare_utf16(a, b) {
        Ordering::Equal => 0.0,
        Ordering::Greater => 1.0,
        Ordering::Less => -1.0,
    }
}

/// `toDateSortValue`: a date's time value, anything else as it is.
fn date_sort_value(value: &Value) -> Value {
    match value {
        Value::Date(time) => Value::Number(*time),
        other => other.clone(),
    }
}

/// The module's `toString`: finite numbers as `String(n)`, strings as they
/// are, everything else (including `NaN` and the infinities) as `""`.
fn sort_string(value: &Value) -> String {
    match value {
        Value::Number(n) if n.is_finite() => js_number(*n),
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

/// `toString(value).toLowerCase()`. Rust's `to_lowercase` applies the same
/// Unicode default case conversion, final sigma included.
fn lower(value: &Value) -> String {
    sort_string(value).to_lowercase()
}

fn is_digit(unit: u16) -> bool {
    (48..=57).contains(&unit)
}

/// `findChunkEnd`.
fn chunk_end(s: &[u16], start: usize, numeric: bool) -> usize {
    let mut end = start + 1;
    while end < s.len() && is_digit(s[end]) == numeric {
        end += 1;
    }
    end
}

/// `compareAlphanumeric`: digit runs compare by value, other runs by code
/// unit, and the string with more chunks left over sorts later.
pub fn compare_alphanumeric(a: &str, b: &str) -> f64 {
    let a: Vec<u16> = a.encode_utf16().collect();
    let b: Vec<u16> = b.encode_utf16().collect();
    let (mut ai, mut bi) = (0, 0);
    while ai < a.len() && bi < b.len() {
        let a_numeric = is_digit(a[ai]);
        let b_numeric = is_digit(b[bi]);
        let a_end = chunk_end(&a, ai, a_numeric);
        let b_end = chunk_end(&b, bi, b_numeric);
        if !a_numeric && !b_numeric {
            let order = a[ai..a_end].cmp(&b[bi..b_end]);
            if order != Ordering::Equal {
                return if order == Ordering::Greater {
                    1.0
                } else {
                    -1.0
                };
            }
            ai = a_end;
            bi = b_end;
            continue;
        }
        if a_numeric != b_numeric {
            return if a_numeric { 1.0 } else { -1.0 };
        }
        let order = compare_numeric_chunks(&a[ai..a_end], &b[bi..b_end]);
        if order != 0.0 {
            return order;
        }
        ai = a_end;
        bi = b_end;
    }
    remaining_chunks(&a, ai) as f64 - remaining_chunks(&b, bi) as f64
}

/// `compareNumericChunks`: exact for up to 15 significant digits, through
/// `parseInt` (to the nearest f64) beyond.
fn compare_numeric_chunks(a: &[u16], b: &[u16]) -> f64 {
    fn significant(s: &[u16]) -> &[u16] {
        let zeros = s.iter().take_while(|&&u| u == 48).count();
        &s[zeros..]
    }
    let a_significant = significant(a);
    let b_significant = significant(b);
    if a_significant.is_empty() && b_significant.is_empty() {
        return 0.0;
    }
    let (an, bn) = if a_significant.len() <= 15 && b_significant.len() <= 15 {
        (small_int(a_significant), small_int(b_significant))
    } else {
        (parse_int(a), parse_int(b))
    };
    if an > bn {
        1.0
    } else if bn > an {
        -1.0
    } else {
        0.0
    }
}

/// `parseSmallInt`: at most 15 digits, so exact in f64.
fn small_int(digits: &[u16]) -> f64 {
    digits
        .iter()
        .fold(0.0, |acc, &unit| acc * 10.0 + f64::from(unit - 48))
}

/// `parseInt(digits, 10)` on a run of ASCII digits: the decimal value
/// rounded to the nearest f64.
fn parse_int(digits: &[u16]) -> f64 {
    let text: String = digits.iter().map(|&u| char::from(u as u8)).collect();
    text.parse::<f64>().expect("digits")
}

/// `countRemainingChunks`.
fn remaining_chunks(s: &[u16], start: usize) -> usize {
    let mut count = 0;
    let mut index = start;
    while index < s.len() {
        count += 1;
        index = chunk_end(s, index, is_digit(s[index]));
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphanumeric_compares_digit_runs_by_value() {
        assert_eq!(compare_alphanumeric("item2", "item10"), -1.0);
        assert_eq!(compare_alphanumeric("item010", "item10"), 0.0);
        assert_eq!(compare_alphanumeric("a1b", "a1"), 1.0);
        assert_eq!(compare_alphanumeric("1", "a"), 1.0);
    }
}
