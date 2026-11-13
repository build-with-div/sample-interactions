//! Panel size props (`defaultSize`, `minSize`, `maxSize`, `collapsedSize`,
//! `collapsedThreshold`): `parseSizeAndUnit` and `sizeStyleToPixels`.

/// A size as the package accepts it: a number is pixels; a string carries
/// its unit, and a string without a known unit is a percentage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    Px(f64),
    Percent(f64),
    Rem(f64),
    Em(f64),
    Vh(f64),
    Vw(f64),
}

impl Size {
    /// `parseSizeAndUnit` for a string. `parseFloat` reads the longest
    /// numeric prefix.
    pub fn parse(text: &str) -> Self {
        let numeric = parse_float(text);
        if text.ends_with('%') {
            Size::Percent(numeric)
        } else if text.ends_with("px") {
            Size::Px(numeric)
        } else if text.ends_with("rem") {
            Size::Rem(numeric)
        } else if text.ends_with("em") {
            Size::Em(numeric)
        } else if text.ends_with("vh") {
            Size::Vh(numeric)
        } else if text.ends_with("vw") {
            Size::Vw(numeric)
        } else {
            Size::Percent(numeric)
        }
    }

    /// `sizeStyleToPixels`. The package reads font sizes and the viewport
    /// from the panel element (`getComputedStyle`, `innerWidth`); the group
    /// passes them in.
    pub fn to_pixels(self, axis_size: f64, units: &UnitContext) -> f64 {
        match self {
            Size::Percent(size) => (size / 100.0) * axis_size,
            Size::Px(size) => size,
            Size::Rem(size) => size * units.root_font_size,
            Size::Em(size) => size * units.font_size,
            Size::Vh(size) => (size / 100.0) * units.viewport_height,
            Size::Vw(size) => (size / 100.0) * units.viewport_width,
        }
    }
}

impl From<f64> for Size {
    fn from(pixels: f64) -> Self {
        Size::Px(pixels)
    }
}

impl From<&str> for Size {
    fn from(text: &str) -> Self {
        Size::parse(text)
    }
}

/// What `rem`, `em`, `vh` and `vw` resolve against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitContext {
    pub root_font_size: f64,
    pub font_size: f64,
    pub viewport_width: f64,
    pub viewport_height: f64,
}

/// JavaScript's `parseFloat`: optional whitespace, then the longest prefix
/// that is a decimal literal or `Infinity`; NaN when there is none.
fn parse_float(text: &str) -> f64 {
    let text = text.trim_start();
    let bytes = text.as_bytes();
    let mut end = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        end = 1;
    }
    if text[end..].starts_with("Infinity") {
        return if text.starts_with('-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    let digits_start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    let mut mantissa_digits = end - digits_start;
    if bytes.get(end) == Some(&b'.') {
        let fraction_start = end + 1;
        let mut fraction_end = fraction_start;
        while bytes.get(fraction_end).is_some_and(u8::is_ascii_digit) {
            fraction_end += 1;
        }
        mantissa_digits += fraction_end - fraction_start;
        if mantissa_digits > 0 {
            end = fraction_end;
        }
    }
    if mantissa_digits == 0 {
        return f64::NAN;
    }
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let mut exponent_end = end + 1;
        if matches!(bytes.get(exponent_end), Some(b'+' | b'-')) {
            exponent_end += 1;
        }
        let exponent_digits_start = exponent_end;
        while bytes.get(exponent_end).is_some_and(u8::is_ascii_digit) {
            exponent_end += 1;
        }
        if exponent_end > exponent_digits_start {
            end = exponent_end;
        }
    }
    text[..end].parse().unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_like_the_package() {
        assert_eq!(Size::parse("25%"), Size::Percent(25.0));
        assert_eq!(Size::parse("200px"), Size::Px(200.0));
        assert_eq!(Size::parse("1.5rem"), Size::Rem(1.5));
        assert_eq!(Size::parse("2em"), Size::Em(2.0));
        assert_eq!(Size::parse("10vh"), Size::Vh(10.0));
        assert_eq!(Size::parse("10vw"), Size::Vw(10.0));
        assert_eq!(Size::parse("30"), Size::Percent(30.0));
        assert_eq!(Size::parse(".5%"), Size::Percent(0.5));
        assert!(matches!(Size::parse("abc"), Size::Percent(value) if value.is_nan()));
        assert_eq!(Size::from(120.0), Size::Px(120.0));
    }
}
