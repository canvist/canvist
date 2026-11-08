//! HTML tokens: tags with their attributes, and decoded text.

/// One piece of markup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Token {
	Open {
		name: String,
		attrs: Vec<(String, String)>,
	},
	Close(String),
	Text(String),
}

/// Splits HTML into tags and decoded text, skipping comments, doctypes,
/// and the contents of `<script>` and `<style>`.
pub(super) struct Tokenizer<'a> {
	rest: &'a str,
}

impl<'a> Tokenizer<'a> {
	pub(super) fn new(html: &'a str) -> Self {
		Self { rest: html }
	}

	/// Skip past `end`, or to the end of the input.
	fn skip_past(&mut self, end: &str) {
		self.rest = self
			.rest
			.find(end)
			.map_or("", |index| &self.rest[index + end.len()..]);
	}

	/// Skip past `end` in any ASCII case, which tag names ignore, or to the
	/// end of the input.
	fn skip_past_tag(&mut self, end: &str) {
		// Lowercasing ASCII keeps every byte where it was.
		let index = self.rest.to_ascii_lowercase().find(end);

		self.rest = index.map_or("", |index| &self.rest[index + end.len()..]);
	}

	/// Skip a comment, doctype, or processing instruction, returning
	/// whether there was one.
	fn skip_markup(&mut self) -> bool {
		if self.rest.starts_with("<!--") {
			self.skip_past("-->");

			return true;
		}

		if self.rest.starts_with("<!") || self.rest.starts_with("<?") {
			self.skip_past(">");

			return true;
		}

		false
	}

	/// Read the tag at the start of the input, opening or closing, and skip
	/// the contents of `script` and `style`, which are never text. `None`
	/// when the tag never ends.
	fn tag(&mut self) -> Option<Token> {
		let closing = self.rest.starts_with("</");
		let body_start = if closing { 2 } else { 1 };
		let end = tag_end(self.rest)?;
		let body = &self.rest[body_start..end];
		self.rest = &self.rest[end + 1..];
		let body = body.trim_end_matches('/');
		let name_end = body
			.find(|character: char| character.is_whitespace())
			.unwrap_or(body.len());
		let name = body[..name_end].to_ascii_lowercase();

		if closing {
			return Some(Token::Close(name));
		}

		if name == "script" || name == "style" {
			self.skip_past_tag(&format!("</{name}"));
			self.skip_past(">");
		}

		Some(Token::Open {
			attrs: parse_attrs(&body[name_end..]),
			name,
		})
	}

	fn text(&mut self) -> Token {
		let first = self.rest.chars().next().map_or(0, char::len_utf8);
		let end = self.rest[first..]
			.find('<')
			.map_or(self.rest.len(), |index| index + first);
		let text = decode(&self.rest[..end]);
		self.rest = &self.rest[end..];
		Token::Text(text)
	}
}

impl Iterator for Tokenizer<'_> {
	type Item = Token;

	/// Skip comments and declarations, then read a tag when `<` starts
	/// one, or text up to the next `<`.
	fn next(&mut self) -> Option<Token> {
		while self.skip_markup() {}

		if self.rest.is_empty() {
			return None;
		}

		let starts_tag = self.rest.starts_with("</")
			|| (self.rest.starts_with('<')
				&& self.rest[1..].starts_with(|next: char| next.is_ascii_alphabetic()));

		if starts_tag {
			return self.tag();
		}

		Some(self.text())
	}
}

/// Where the tag at the start of `html` ends: the first `>` outside a
/// quoted attribute value, as browsers read it, or the first `>` of all
/// when a quote never closes.
fn tag_end(html: &str) -> Option<usize> {
	let mut quote = None;
	let mut after_equals = false;

	for (index, byte) in html.bytes().enumerate() {
		match (quote, byte, after_equals) {
			(Some(open), ..) => quote = (byte != open).then_some(open),
			(None, b'"' | b'\'', true) => quote = Some(byte),
			(None, b'>', _) => return Some(index),
			_ => {}
		}

		after_equals =
			quote.is_none() && (byte == b'=' || (after_equals && byte.is_ascii_whitespace()));
	}

	html.find('>')
}

/// Parse `name="value"` pairs, with single, double, or no quotes.
fn parse_attrs(source: &str) -> Vec<(String, String)> {
	let mut attrs = Vec::new();
	let mut rest = source.trim_start();

	while !rest.is_empty() {
		let name_end = rest
			.find(|character: char| character.is_whitespace() || character == '=')
			.unwrap_or(rest.len());
		let name = rest[..name_end].to_ascii_lowercase();
		rest = rest[name_end..].trim_start();

		let value = match rest.strip_prefix('=') {
			Some(after) => {
				let (value, remaining) = attr_value(after.trim_start());
				rest = remaining.trim_start();
				value
			}
			None => String::new(),
		};

		if !name.is_empty() {
			attrs.push((name, value));
		}
	}

	attrs
}

/// A quoted or bare attribute value and the input after it.
fn attr_value(source: &str) -> (String, &str) {
	let quote = source
		.chars()
		.next()
		.filter(|&quote| quote == '"' || quote == '\'');

	let Some(quote) = quote else {
		let end = source.find(char::is_whitespace).unwrap_or(source.len());

		return (decode(&source[..end]), &source[end..]);
	};
	let body = &source[1..];
	let end = body.find(quote).unwrap_or(body.len());
	let rest = body.get(end + 1..).unwrap_or("");

	(decode(&body[..end]), rest)
}

/// Decode character references.
pub(super) fn decode(text: &str) -> String {
	let mut decoded = String::with_capacity(text.len());
	let mut rest = text;

	while let Some(start) = rest.find('&') {
		decoded.push_str(&rest[..start]);
		rest = &rest[start..];
		let end = rest.find(';').filter(|&end| end <= 10);
		let character = end.and_then(|end| entity(&rest[1..end]));

		if let (Some(end), Some(character)) = (end, character) {
			decoded.push(character);
			rest = &rest[end + 1..];
		} else {
			decoded.push('&');
			rest = &rest[1..];
		}
	}

	decoded.push_str(rest);
	decoded
}

/// The radix of a `&#x…;` character reference.
const HEXADECIMAL: u32 = 16;

/// The radix of a `&#…;` character reference.
const DECIMAL: u32 = 10;

/// The character a named or numeric character reference stands for.
fn entity(name: &str) -> Option<char> {
	let numeric = |digits: &str, radix| {
		u32::from_str_radix(digits, radix)
			.ok()
			.and_then(char::from_u32)
	};

	match name {
		"amp" => Some('&'),
		"lt" => Some('<'),
		"gt" => Some('>'),
		"quot" => Some('"'),
		"apos" => Some('\''),
		"nbsp" => Some('\u{a0}'),
		_ => {
			let digits = name.strip_prefix('#')?;

			match digits.strip_prefix(['x', 'X']) {
				Some(hex) => numeric(hex, HEXADECIMAL),
				None => numeric(digits, DECIMAL),
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn tokenizes_tags_attributes_and_entities() {
		let tokens: Vec<Token> =
			Tokenizer::new("<!-- c --><a HREF='x&amp;y' data-x=1 hidden>&lt;3 &#x1F600;</a>")
				.collect();

		assert_eq!(
			tokens,
			[
				Token::Open {
					name: "a".to_owned(),
					attrs: vec![
						("href".to_owned(), "x&y".to_owned()),
						("data-x".to_owned(), "1".to_owned()),
						("hidden".to_owned(), String::new()),
					],
				},
				Token::Text("<3 😀".to_owned()),
				Token::Close("a".to_owned()),
			]
		);
	}

	#[test]
	fn skips_scripts_styles_and_doctypes() {
		let tokens: Vec<Token> =
			Tokenizer::new("<!DOCTYPE html><style>p{}</style><script>if (a < b) {}</script>x")
				.collect();

		assert_eq!(tokens.last(), Some(&Token::Text("x".to_owned())));
		assert_eq!(tokens.len(), 3);
	}

	#[test]
	fn keeps_unknown_entities_and_stray_ampersands() {
		assert_eq!(decode("a & b &bogus; &#xZZ;"), "a & b &bogus; &#xZZ;");
	}
}
