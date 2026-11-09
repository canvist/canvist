//! A deliberately small CSS value grammar for HTML export. Document strings
//! can come from JSON or collaboration, so HTML escaping alone is insufficient.
//! Functions with nested values (including `var` and `url`), escapes, comments,
//! and declaration delimiters are outside this grammar.

/// A value that can only contribute tokens to the requested property. Keep the
/// original spelling for clipboard round trips; HTML-escape it when writing it.
pub(super) fn value<'a>(property: &str, raw: &'a str) -> Option<&'a str> {
	if raw.chars().any(char::is_control) {
		return None;
	}

	let value = raw.trim();
	let lower = value.to_ascii_lowercase();

	let supported = match property {
		"text-align" => matches!(lower.as_str(), "left" | "center" | "right" | "justify"),
		"color" | "background-color" => color(&lower),
		"font-family" => font_family(value),
		"font-size" | "line-height" => size(property, &lower),
		_ => false,
	};

	supported.then_some(value)
}

/// The digits a hex colour may have: `#rgb`, `#rgba`, `#rrggbb`, and
/// `#rrggbbaa`.
const HEX_LENGTHS: [usize; 4] = [3, 4, 6, 8];

/// Hex and named colours, plus flat numeric colour functions. Restrict function
/// names and their argument alphabet so a value can never reference a resource.
fn color(value: &str) -> bool {
	if let Some(hex) = value.strip_prefix('#') {
		return HEX_LENGTHS.contains(&hex.len())
			&& hex.bytes().all(|byte| byte.is_ascii_hexdigit());
	}

	if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
		return true;
	}

	let Some((function, args)) = value.split_once('(') else {
		return false;
	};
	let Some(args) = args.strip_suffix(')') else {
		return false;
	};

	matches!(
		function,
		"rgb" | "rgba" | "hsl" | "hsla" | "hwb" | "lab" | "lch" | "oklab" | "oklch" | "color"
	) && !args.is_empty()
		&& args.bytes().all(|byte| {
			byte.is_ascii_alphanumeric()
				|| matches!(byte, b' ' | b'.' | b',' | b'%' | b'/' | b'+' | b'-')
		})
}

/// Family lists made of plain names or quoted names. Quotes must enclose a
/// whole family, and names cannot contain CSS syntax even inside quotes.
fn font_family(value: &str) -> bool {
	value.split(',').all(|family| {
		let family = family.trim();

		let name = match family.chars().next() {
			Some(quote @ ('\'' | '"')) => {
				family
					.strip_prefix(quote)
					.and_then(|name| name.strip_suffix(quote))
			}
			_ => Some(family),
		};

		name.is_some_and(|name| {
			!name.trim().is_empty()
				&& name.chars().all(|character| {
					character.is_alphanumeric() || matches!(character, ' ' | '-' | '_')
				})
		})
	})
}

/// Finite, nonnegative dimensions in supported CSS units, unitless line height,
/// and the corresponding property keywords. Expressions are deliberately absent.
fn size(property: &str, value: &str) -> bool {
	let keywords: &[&str] = match property {
		"font-size" => &FONT_SIZE_KEYWORDS,
		_ => &["normal"],
	};

	if GLOBAL_KEYWORDS.contains(&value) || keywords.contains(&value) {
		return true;
	}

	let end = value
		.find(|character: char| !character.is_ascii_digit() && character != '.')
		.unwrap_or(value.len());
	let (number, unit) = value.split_at(end);
	let Ok(number) = number.parse::<f64>() else {
		return false;
	};

	if !number.is_finite() || number < 0.0 {
		return false;
	}

	if unit.is_empty() {
		return property == "line-height" || number == 0.0;
	}

	UNITS.contains(&unit)
}

/// The keywords every property takes.
const GLOBAL_KEYWORDS: [&str; 5] = ["inherit", "initial", "unset", "revert", "revert-layer"];

/// The keyword sizes of `font-size`.
const FONT_SIZE_KEYWORDS: [&str; 10] = [
	"xx-small",
	"x-small",
	"small",
	"medium",
	"large",
	"x-large",
	"xx-large",
	"xxx-large",
	"smaller",
	"larger",
];

/// The length and percentage units sizes may use.
const UNITS: [&str; 18] = [
	"px", "em", "rem", "%", "ex", "ch", "lh", "rlh", "vw", "vh", "vmin", "vmax", "pt", "pc", "cm",
	"mm", "in", "q",
];
