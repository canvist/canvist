use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_core::Extension;
use canvist_core::TypedExtension;
use canvist_extension_document::Document;
use canvist_extension_paragraph::Paragraph;
use canvist_model::Schema;

use super::*;

static CLIP: MediaBlock = MediaBlock {
	name: "clip",
	command: "setClip",
	doc: "Insert a clip at the selection.",
	attrs: &[
		MediaAttr::text("src", "The clip URL."),
		MediaAttr::text("title", "A title."),
		MediaAttr::number("width", Some(640), "The width in pixels."),
		MediaAttr::flag("muted", false, "Start muted."),
	],
	accepts: Some(|src| src.starts_with("https://")),
};

struct Clip;

impl Extension for Clip {
	fn name(&self) -> &'static str {
		CLIP.name
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		CLIP.schema(schema)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		CLIP.nodes()
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(CLIP.commands())
	}
}

impl TypedExtension for Clip {
	type Handle = MediaHandle;

	fn handle(&self, registration: Registration) -> MediaHandle {
		CLIP.handle(registration)
	}
}

fn editor() -> Result<(Editor, MediaHandle), EditorError> {
	let mut builder = Editor::builder().extension(Document).extension(Paragraph);
	let clip = builder.add(Clip);
	Ok((builder.build()?, clip))
}

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

/// The attributes every clip in the document stores.
fn clips(editor: &Editor) -> Vec<Attrs> {
	let doc = editor.state().doc();
	let kind = doc.schema().block_type(CLIP.name);
	doc.iter()
		.filter(|block| Some(block.kind()) == kind)
		.map(|block| block.attrs().clone())
		.collect()
}

#[test]
fn only_accepted_sources_insert() -> Result<(), EditorError> {
	let (mut editor, clip) = editor()?;
	let refused = clip.insert("javascript:alert(1)", &Attrs::new());
	assert!(editor.run(&refused, &mut env())?.is_none());

	let args = Attrs::new().with("src", "http://example.com/a");
	assert!(editor.run_named("setClip", &args, &mut env())?.is_none());
	assert!(clips(&editor).is_empty());

	let accepted = clip.insert("https://example.com/a", &Attrs::new());
	assert!(editor.run(&accepted, &mut env())?.is_some());
	assert_eq!(
		clips(&editor),
		[Attrs::new().with("src", "https://example.com/a")]
	);
	Ok(())
}

#[test]
fn inserting_stores_only_known_well_typed_and_changed_attributes() -> Result<(), EditorError> {
	let (mut editor, clip) = editor()?;
	let given = Attrs::new()
		.with("title", "A")
		.with("width", 640)
		.with("muted", Value::Null)
		.with("nonsense", true);
	editor.run(&clip.insert("https://a", &given), &mut env())?;

	let given = Attrs::new()
		.with("title", 7)
		.with("width", 320.5)
		.with("muted", true)
		.with("src", "https://ignored");
	editor.run(&clip.insert("https://b", &given), &mut env())?;

	assert_eq!(
		clips(&editor),
		[
			Attrs::new().with("title", "A").with("src", "https://a"),
			Attrs::new()
				.with("src", "https://b")
				.with("width", 320.5)
				.with("muted", true),
		]
	);
	Ok(())
}

#[test]
fn the_command_requires_the_source_and_checks_the_rest() -> Result<(), EditorError> {
	let (mut editor, _) = editor()?;
	let missing = editor.run_named("setClip", &Attrs::new(), &mut env());
	assert!(matches!(missing, Err(EditorError::InvalidArgs { .. })));

	let mistyped = Attrs::new().with("src", "https://a").with("width", "wide");
	let mistyped = editor.run_named("setClip", &mistyped, &mut env());
	assert!(matches!(mistyped, Err(EditorError::InvalidArgs { .. })));

	let args = Attrs::new().with("src", "https://a").with("width", 640);
	assert!(editor.run_named("setClip", &args, &mut env())?.is_some());
	assert_eq!(clips(&editor), [Attrs::new().with("src", "https://a")]);
	Ok(())
}
