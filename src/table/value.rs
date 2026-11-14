//! Cell values with the JavaScript semantics table-core's sorting relies on:
//! strict equality, the relational operators and `ToNumber`.
//!
//! Rows are JavaScript objects in TanStack Table, and its built-in sort
//! functions compare whatever the accessors return with `===`, `<` and `>`.
//! The port keeps those operators exact, including across types (a number
//! against a string converts the string; `NaN` is unordered), because they
//! decide the row order.

use std::cmp::Ordering;

use crate::panels::layout::js_number;

/// A JavaScript value as row data carries it.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    /// A `Date` object, by its time value (`NaN` for an invalid date).
    Date(f64),
    Array(Vec<Value>),
    /// A plain object; its properties in insertion order.
    Object(Vec<(String, Value)>),
}

impl Value {
    /// The property access `value[key]` for the values row data holds: an
    /// object's own property, an array's element or `length`, otherwise
    /// `undefined`.
    pub fn get(&self, key: &str) -> Value {
        match self {
            Value::Object(entries) => entries
                .iter()
                .rev()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.clone())
                .unwrap_or(Value::Undefined),
            Value::Array(items) if key == "length" => Value::Number(items.len() as f64),
            Value::Array(items) => array_index(key)
                .and_then(|index| items.get(index))
                .cloned()
                .unwrap_or(Value::Undefined),
            _ => Value::Undefined,
        }
    }

    /// `value === undefined`.
    pub fn is_undefined(&self) -> bool {
        matches!(self, Value::Undefined)
    }

    /// `value == null`.
    pub fn is_nullish(&self) -> bool {
        matches!(self, Value::Undefined | Value::Null)
    }

    /// `typeof value === 'string'`.
    pub fn is_string(&self) -> bool {
        matches!(self, Value::String(_))
    }
}

/// A canonical array index (`"0"`, `"17"`, not `"01"`), as property keys
/// name array elements.
pub(crate) fn array_index(key: &str) -> Option<usize> {
    if key.is_empty() || (key.len() > 1 && key.starts_with('0')) {
        return None;
    }
    if !key.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    key.parse::<u32>()
        .ok()
        .filter(|index| *index != u32::MAX)
        .map(|index| index as usize)
}

/// `a === b`. Objects (dates, arrays, plain objects) are equal only to
/// themselves, and no two values here are the same object.
pub fn strict_equals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Undefined, Value::Undefined) | (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        _ => false,
    }
}

/// `a > b`.
pub fn greater_than(a: &Value, b: &Value) -> bool {
    less_than(b, a) == Some(true)
}

/// `a < b` as a boolean.
pub fn less(a: &Value, b: &Value) -> bool {
    less_than(a, b) == Some(true)
}

/// IsLessThan(a, b) (ECMA-262 7.2.13): `Some` when the values are ordered,
/// `None` when either converts to `NaN`.
fn less_than(a: &Value, b: &Value) -> Option<bool> {
    let a = to_primitive(a);
    let b = to_primitive(b);
    if let (Primitive::String(a), Primitive::String(b)) = (&a, &b) {
        return Some(compare_utf16(a, b) == Ordering::Less);
    }
    let a = primitive_to_number(&a);
    let b = primitive_to_number(&b);
    if a.is_nan() || b.is_nan() {
        None
    } else {
        Some(a < b)
    }
}

enum Primitive {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(String),
}

/// ToPrimitive with hint number: a date's time value, an array's `join()`,
/// `"[object Object]"` for a plain object.
fn to_primitive(value: &Value) -> Primitive {
    match value {
        Value::Undefined => Primitive::Undefined,
        Value::Null => Primitive::Null,
        Value::Bool(b) => Primitive::Bool(*b),
        Value::Number(n) => Primitive::Number(*n),
        Value::String(s) => Primitive::String(s.clone()),
        Value::Date(time) => Primitive::Number(*time),
        Value::Array(_) | Value::Object(_) => Primitive::String(to_js_string(value)),
    }
}

fn primitive_to_number(value: &Primitive) -> f64 {
    match value {
        Primitive::Undefined => f64::NAN,
        Primitive::Null => 0.0,
        Primitive::Bool(b) => f64::from(u8::from(*b)),
        Primitive::Number(n) => *n,
        Primitive::String(s) => string_to_number(s),
    }
}

/// `String(value)`.
///
/// # Panics
///
/// On a date: `Date.prototype.toString` depends on the time zone, and
/// nothing the port runs needs it.
pub fn to_js_string(value: &Value) -> String {
    match value {
        Value::Undefined => "undefined".into(),
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => js_number(*n),
        Value::String(s) => s.clone(),
        Value::Date(_) => panic!("String(date) depends on the time zone and is not ported"),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                if item.is_nullish() {
                    String::new()
                } else {
                    to_js_string(item)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// Code-unit order, as JavaScript compares strings.
pub fn compare_utf16(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// StringToNumber (ECMA-262 7.1.4.1.1): `Number(" 12 ")` is 12, `""` is 0,
/// `"0x1F"` is 31 and anything else that isn't a numeric literal is `NaN`.
pub fn string_to_number(text: &str) -> f64 {
    let text = text.trim_matches(is_js_whitespace);
    if text.is_empty() {
        return 0.0;
    }
    let bytes = text.as_bytes();
    if bytes.len() > 2 && bytes[0] == b'0' {
        let radix = match bytes[1] {
            b'x' | b'X' => 16,
            b'o' | b'O' => 8,
            b'b' | b'B' => 2,
            _ => 0,
        };
        if radix != 0 {
            return non_decimal(&text[2..], radix);
        }
    }
    let (sign, unsigned) = match bytes[0] {
        b'+' => (1.0, &text[1..]),
        b'-' => (-1.0, &text[1..]),
        _ => (1.0, text),
    };
    if unsigned == "Infinity" {
        return sign * f64::INFINITY;
    }
    if !is_decimal_literal(unsigned) {
        return f64::NAN;
    }
    // Rust's parser rounds correctly, as JavaScript's does.
    sign * unsigned.parse::<f64>().expect("validated decimal literal")
}

/// StrUnsignedDecimalLiteral without `Infinity`: digits with an optional
/// fraction (either part may be empty, not both) and exponent.
fn is_decimal_literal(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    let digits = |i: &mut usize| {
        let start = *i;
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
        *i - start
    };
    let whole = digits(&mut i);
    let mut fraction = 0;
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        fraction = digits(&mut i);
    }
    if whole + fraction == 0 {
        return false;
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        if digits(&mut i) == 0 {
            return false;
        }
    }
    i == bytes.len()
}

/// A `0x`, `0o` or `0b` literal's digits, rounded to the nearest f64.
fn non_decimal(digits: &str, radix: u32) -> f64 {
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return f64::NAN;
    }
    match u128::from_str_radix(digits, radix) {
        // `as` rounds to nearest, ties to even, like the spec's MV rounding.
        Ok(value) => value as f64,
        Err(_) => {
            // Beyond 128 bits: no table value reaches this; accumulate.
            digits.chars().fold(0.0, |acc, c| {
                acc * f64::from(radix) + f64::from(c.to_digit(radix).unwrap())
            })
        }
    }
}

/// WhiteSpace and LineTerminator (ECMA-262 12.2, 12.3), which `Number()`
/// trims.
fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{9}' | '\u{A}' | '\u{B}' | '\u{C}' | '\u{D}' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_to_number_follows_the_grammar() {
        assert_eq!(string_to_number(" 12 "), 12.0);
        assert_eq!(string_to_number(""), 0.0);
        assert_eq!(string_to_number("0x1F"), 31.0);
        assert_eq!(string_to_number("1e2"), 100.0);
        assert_eq!(string_to_number("-4.5"), -4.5);
        assert_eq!(string_to_number(".5"), 0.5);
        assert_eq!(string_to_number("5."), 5.0);
        assert_eq!(string_to_number("-Infinity"), f64::NEG_INFINITY);
        assert!(string_to_number("abc").is_nan());
        assert!(string_to_number("inf").is_nan());
        assert!(string_to_number("-0x1").is_nan());
        assert!(string_to_number("1e").is_nan());
        assert!(string_to_number(".").is_nan());
    }

    #[test]
    fn relational_operators_convert_like_javascript() {
        let s = |t: &str| Value::String(t.into());
        assert!(greater_than(&s("b"), &s("a")));
        assert!(greater_than(&s(" 12 "), &Value::Number(5.0)));
        assert!(!greater_than(&s("abc"), &Value::Number(5.0)));
        assert!(!less(&s("abc"), &Value::Number(5.0)));
        assert!(greater_than(&Value::Bool(true), &Value::Null));
        // U+FB01 sorts after U+1F600's surrogates in UTF-16.
        assert!(greater_than(&s("a\u{FB01}"), &s("a\u{1F600}")));
    }
}
