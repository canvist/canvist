//! HTML export validates styles even when they arrived through document JSON.

#![cfg(feature = "full")]

mod support;

use canvist_core::Slice;
use canvist_extensions::clipboard;
use canvist_model::Document;
use serde_json::Value as Json;
use serde_json::json;

/// Export schema-valid JSON without passing through the HTML importer.
fn export(block: &Json) -> String {
	let editor = support::editor_with(canvist_extensions::extras());
	let schema = editor.schema();
	let doc = Document::from_json(
		schema.clone(),
		1,
		&json!({
			"type": "doc",
			"content": [block],
		}),
	)
	.expect("valid document JSON");
	let blocks = doc
		.children(None)
		.iter()
		.map(|&id| doc.tree(id).expect("block"))
		.collect();

	clipboard::to_html(schema, &Slice::new(blocks))
}

/// A paragraph with one marked text run.
fn marked(name: &str, attrs: &Json) -> Json {
	json!({
		"type": "paragraph",
		"content": [{"type": "text", "text": "safe", "marks": [{"type": name, "attrs": attrs}]}],
	})
}

#[test]
fn html_export_omits_css_injection_from_every_style_attribute() {
	let payloads = [
		"red; background-image: url(https://invalid.example/track)",
		"red} body {position:fixed}",
		"url(https://invalid.example/track)",
		r"r\65 d",
		"red/**/",
		"red\n",
		"red\r",
		"red\t",
		"red\0",
		"red !important",
		"var(--untrusted)",
		"expression(alert(1))",
		"'unterminated",
	];

	for payload in payloads {
		for attr in [
			"color",
			"backgroundColor",
			"fontFamily",
			"fontSize",
			"lineHeight",
		] {
			assert_eq!(
				export(&marked("textStyle", &json!({attr: payload}))),
				"<p><span>safe</span></p>",
				"{attr}: {payload:?}",
			);
		}

		assert_eq!(
			export(&marked("highlight", &json!({"color": payload}))),
			"<p><mark>safe</mark></p>",
			"highlight: {payload:?}",
		);

		for (kind, tag) in [("paragraph", "p"), ("heading", "h1")] {
			assert_eq!(
				export(&json!({
					"type": kind,
					"attrs": {"textAlign": payload},
					"content": [{"type": "text", "text": "safe"}],
				})),
				format!("<{tag}>safe</{tag}>"),
				"{kind} alignment: {payload:?}",
			);
		}
	}
}

#[test]
fn html_export_preserves_supported_styles() {
	let styles = [
		("color", "color", "#f08"),
		("color", "color", "#ff008080"),
		("color", "color", "rebeccapurple"),
		("color", "color", "rgb(1, 2, 3)"),
		("color", "color", "rgba(1, 2, 3, 0.5)"),
		("color", "color", "hsl(120deg 50% 50% / 0.5)"),
		("color", "color", "oklch(50% 0.2 120)"),
		("backgroundColor", "background-color", "transparent"),
		("fontFamily", "font-family", "Inter, sans-serif"),
		("fontFamily", "font-family", "'Noto Sans', serif"),
		("fontFamily", "font-family", "\"Noto Sans\", serif"),
		("fontFamily", "font-family", "日本語, sans-serif"),
		("fontSize", "font-size", "18px"),
		("fontSize", "font-size", "1.25rem"),
		("fontSize", "font-size", "120%"),
		("fontSize", "font-size", "larger"),
		("fontSize", "font-size", "0"),
		("lineHeight", "line-height", "1.5"),
		("lineHeight", "line-height", "24px"),
		("lineHeight", "line-height", "normal"),
	];

	for (attr, property, value) in styles {
		let escaped = value.replace('"', "&quot;");
		assert_eq!(
			export(&marked("textStyle", &json!({attr: value}))),
			format!("<p><span style=\"{property}: {escaped}\">safe</span></p>"),
			"{attr}: {value}",
		);
	}

	for align in ["left", "center", "right", "justify"] {
		assert_eq!(
			export(&json!({
				"type": "paragraph",
				"attrs": {"textAlign": align},
				"content": [{"type": "text", "text": "safe"}],
			})),
			format!("<p style=\"text-align: {align}\">safe</p>"),
		);
	}

	assert_eq!(
		export(&marked("highlight", &json!({"color": "#fef08a"}))),
		"<p><mark data-color=\"#fef08a\" style=\"background-color: #fef08a; color: \
		 inherit\">safe</mark></p>",
	);
}

#[test]
fn html_export_keeps_valid_styles_when_another_is_rejected() {
	assert_eq!(
		export(&marked(
			"textStyle",
			&json!({
				"color": "red; position: fixed",
				"fontFamily": "Inter, sans-serif",
				"fontSize": "18px",
			})
		)),
		"<p><span style=\"font-family: Inter, sans-serif; font-size: 18px\">safe</span></p>",
	);

	for attr in ["fontSize", "lineHeight"] {
		for value in ["-1px", "NaN", "inf", "1e999px", "1quack", "1..2px", ""] {
			assert_eq!(
				export(&marked("textStyle", &json!({attr: value}))),
				"<p><span>safe</span></p>",
				"{attr}: {value:?}",
			);
		}
	}
}
