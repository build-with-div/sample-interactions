//! Layout number rounding, ported from react-resizable-panels'
//! `formatLayoutNumber`, `layoutNumbersEqual` and `compareLayoutNumbers`.
//! Layout sizes are percentages compared at three decimals.

use std::cmp::Ordering;

/// `parseFloat(number.toFixed(3))`.
///
/// `Number.prototype.toFixed` picks the nearest multiple of 0.001 to the
/// exact binary value and, on a tie, the larger one (ECMA-262
/// 21.1.3.3). This works on the magnitude and keeps the sign, as toFixed
/// does. Rust's `{:.80}` prints the exact decimal expansion for any value
/// where the third decimal can change, so its fourth digit decides.
pub fn format_layout_number(number: f64) -> f64 {
    if !number.is_finite() || number.abs() >= 1e21 {
        return number;
    }
    let digits = format!("{:.80}", number.abs());
    let (whole, fraction) = digits.split_once('.').expect("fixed-point output");
    let mut thousandths: u128 = whole.parse::<u128>().expect("integer part") * 1000
        + fraction[..3].parse::<u128>().expect("three decimals");
    if fraction.as_bytes()[3] >= b'5' {
        thousandths += 1;
    }
    let text = format!(
        "{}{}.{:03}",
        if number < 0.0 { "-" } else { "" },
        thousandths / 1000,
        thousandths % 1000
    );
    text.parse().expect("decimal")
}

/// `layoutNumbersEqual(actual, expected, minimumDelta)`.
pub fn layout_numbers_equal(actual: f64, expected: f64, minimum_delta: f64) -> bool {
    (format_layout_number(actual) - format_layout_number(expected)).abs() <= minimum_delta
}

/// `compareLayoutNumbers(actual, expected)`.
pub fn compare_layout_numbers(actual: f64, expected: f64) -> Ordering {
    if layout_numbers_equal(actual, expected, 0.0) {
        Ordering::Equal
    } else if actual > expected {
        Ordering::Greater
    } else {
        Ordering::Less
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_like_to_fixed() {
        assert_eq!(format_layout_number(0.0625), 0.063); // exact tie: larger
        assert_eq!(format_layout_number(-0.0625), -0.063);
        assert_eq!(format_layout_number(1.0005), 1.0); // 1.000499999...
        assert_eq!(format_layout_number(33.33333333), 33.333);
        assert_eq!(format_layout_number(66.6666666), 66.667);
        assert_eq!(format_layout_number(100.0), 100.0);
    }
}
