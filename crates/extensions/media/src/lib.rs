#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extension_media`
//!
//! The building block of canvist's media extensions: images, YouTube and
//! Twitch videos, audio, and file attachments.
//!
//! Each is an atomic block whose attributes say what to show. The canvas
//! reserves a slot for it, and the host draws the picture, player, or file
//! card there, as a node view does in TipTap. A [`MediaBlock`] describes
//! one, so an extension for another kind of media is a few lines of data:
//!
//! ```
//! use canvist_core::CommandSpec;
//! use canvist_core::Editor;
//! use canvist_core::EditorError;
//! use canvist_core::Extension;
//! use canvist_core::NodeMeta;
//! use canvist_core::Registration;
//! use canvist_core::TypedExtension;
//! use canvist_extension_media::MediaAttr;
//! use canvist_extension_media::MediaBlock;
//! use canvist_extension_media::MediaHandle;
//! use canvist_model::Attrs;
//! use canvist_model::BlockSpec;
//! use canvist_model::Schema;
//! use canvist_model::SchemaBuilder;
//! # use std::time::Duration;
//! # use canvist_core::Env;
//!
//! static VIMEO: MediaBlock = MediaBlock {
//! 	name: "vimeo",
//! 	command: "setVimeoVideo",
//! 	doc: "Insert a Vimeo video at the selection.",
//! 	attrs: &[
//! 		MediaAttr::text("src", "The video URL."),
//! 		MediaAttr::number("width", Some(640), "The width in pixels."),
//! 	],
//! 	accepts: Some(|src| src.starts_with("https://vimeo.com/")),
//! };
//!
//! struct Vimeo;
//!
//! impl Extension for Vimeo {
//! 	fn name(&self) -> &'static str {
//! 		VIMEO.name
//! 	}
//!
//! 	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
//! 		VIMEO.schema(schema.block(BlockSpec::text("paragraph")))
//! 	}
//!
//! 	fn nodes(&self) -> Vec<NodeMeta> {
//! 		VIMEO.nodes()
//! 	}
//!
//! 	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
//! 		Ok(VIMEO.commands())
//! 	}
//! }
//!
//! impl TypedExtension for Vimeo {
//! 	type Handle = MediaHandle;
//!
//! 	fn handle(&self, registration: Registration) -> MediaHandle {
//! 		VIMEO.handle(registration)
//! 	}
//! }
//!
//! let mut builder = Editor::builder();
//! let vimeo = builder.add(Vimeo);
//! let mut editor = builder.build()?;
//! # let mut env = Env::headless(Duration::ZERO);
//!
//! let inserted = editor.run(&vimeo.insert("https://vimeo.com/1", &Attrs::new()), &mut env)?;
//! assert!(inserted.is_some());
//!
//! let refused = editor.run(&vimeo.insert("javascript:alert(1)", &Attrs::new()), &mut env)?;
//! assert!(refused.is_none());
//! # Ok::<(), EditorError>(())
//! ```

use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::Field;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::Registration;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::SchemaBuilder;
use canvist_model::Value;
pub use canvist_model::video_kind;

/// The value a media attribute starts with.
#[derive(Clone, Copy, Debug)]
enum Initial {
	Null,
	Bool(bool),
	/// Small enough to keep the enum compact: pixel sizes and seconds.
	Int(u16),
}

/// What a media attribute holds.
#[derive(Clone, Copy, Debug)]
enum Kind {
	Text,
	Number,
	Flag,
}

/// One attribute of a media block.
#[derive(Clone, Copy, Debug)]
pub struct MediaAttr {
	name: &'static str,
	kind: Kind,
	default: Initial,
	doc: &'static str,
}

impl MediaAttr {
	/// A text attribute, null until set, such as a source URL or alt text.
	#[must_use]
	pub const fn text(name: &'static str, doc: &'static str) -> Self {
		Self {
			name,
			kind: Kind::Text,
			default: Initial::Null,
			doc,
		}
	}

	/// A number, such as a width in pixels, that starts at `default` or
	/// null.
	#[must_use]
	pub const fn number(name: &'static str, default: Option<u16>, doc: &'static str) -> Self {
		let default = match default {
			Some(number) => Initial::Int(number),
			None => Initial::Null,
		};

		Self {
			name,
			kind: Kind::Number,
			default,
			doc,
		}
	}

	/// A flag, such as whether a player starts muted.
	#[must_use]
	pub const fn flag(name: &'static str, default: bool, doc: &'static str) -> Self {
		Self {
			name,
			kind: Kind::Flag,
			default: Initial::Bool(default),
			doc,
		}
	}

	/// The attribute's name.
	#[must_use]
	pub const fn name(self) -> &'static str {
		self.name
	}

	/// The value the attribute starts with.
	#[must_use]
	pub fn default_value(self) -> Value {
		match self.default {
			Initial::Null => Value::Null,
			Initial::Bool(flag) => Value::Bool(flag),
			Initial::Int(number) => Value::Int(i64::from(number)),
		}
	}

	/// The attribute's type, never null.
	fn present_type(self) -> ValueType {
		match self.kind {
			Kind::Text => ValueType::String,
			Kind::Number => ValueType::Float,
			Kind::Flag => ValueType::Bool,
		}
	}

	fn value_type(self) -> ValueType {
		match self.default {
			Initial::Null => ValueType::nullable(self.present_type()),
			_ => self.present_type(),
		}
	}

	/// The attribute as a node attribute, or as a command argument: the
	/// first attribute, the source, is required there, and a required
	/// argument can't be null even where the attribute defaults to it.
	fn field(self, required: bool) -> Field {
		if required {
			Field::required(self.name, self.present_type(), self.doc)
		} else {
			Field::optional(self.name, self.value_type(), self.default_value(), self.doc)
		}
	}
}

/// Everything that distinguishes one media block from another.
#[derive(Debug)]
pub struct MediaBlock {
	/// The block and extension name, as in TipTap.
	pub name: &'static str,
	/// The command that inserts one.
	pub command: &'static str,
	/// What the command does.
	pub doc: &'static str,
	/// The attributes; the first is the source, which the command needs.
	pub attrs: &'static [MediaAttr],
	/// Which sources the block may show, when not every URL is safe to,
	/// such as a video player's `<iframe>`. Inserting another source does
	/// not apply.
	pub accepts: Option<fn(&str) -> bool>,
}

/// Typed access to a media extension, such as an image.
#[derive(Clone, Copy, Debug)]
pub struct MediaHandle {
	block: &'static MediaBlock,
	_registration: Registration,
}

impl MediaHandle {
	/// Insert a block showing `src`, with these other attributes, at the
	/// selection, as a horizontal rule is. Unknown attributes are dropped.
	#[must_use]
	pub fn insert(self, src: &str, attrs: &Attrs) -> Command {
		self.block.insert(src, attrs)
	}
}

impl MediaBlock {
	/// The command inserting `src` with `attrs`. Attributes the type lacks
	/// are dropped, and so are ones equal to their default, since an absent
	/// attribute is its default. A source the block does not
	/// [accept](Self::accepts) does not apply.
	#[must_use]
	pub fn insert(&'static self, src: &str, attrs: &Attrs) -> Command {
		if self.accepts.is_some_and(|accepts| !accepts(src)) {
			return command(|_| Ok(None));
		}

		let source = self.attrs[0].name;
		let mut attrs: Attrs = attrs
			.iter()
			.filter(|(key, value)| {
				self.attrs
					.iter()
					.any(|attr| attr.name == *key && attr.default_value() != **value)
			})
			.map(|(key, value)| (key, value.clone()))
			.collect();
		attrs.set(source, src);

		command(move |state| {
			let kind = canvist_core::block_type(state.schema(), self.name, self.name)?;
			commands::insert_leaf(state, kind, attrs.clone())
		})
	}

	/// The block, for [`Extension::schema`](canvist_core::Extension::schema).
	#[must_use]
	pub fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		let attrs = self
			.attrs
			.iter()
			.map(|attr| (attr.name, attr.default_value()))
			.collect();
		schema.block(BlockSpec::leaf(self.name).attrs(attrs))
	}

	/// The block and its attributes, for
	/// [`Extension::nodes`](canvist_core::Extension::nodes).
	#[must_use]
	pub fn nodes(&self) -> Vec<NodeMeta> {
		let node = NodeMeta::new(NodeKind::Block, self.name);
		vec![
			self.attrs
				.iter()
				.fold(node, |node, attr| node.attr(attr.field(false))),
		]
	}

	/// The insert command, for
	/// [`Extension::commands`](canvist_core::Extension::commands).
	#[must_use]
	pub fn commands(&'static self) -> Vec<CommandSpec> {
		let spec = CommandSpec::new(self.command, self.doc, move |args| {
			let source = args.string(self.attrs[0].name)?;
			let attrs: Attrs = self.attrs[1..]
				.iter()
				.map(|attr| Ok((attr.name, args.get(attr.name)?.clone())))
				.collect::<Result<_, EditorError>>()?;
			Ok(self.insert(source, &attrs))
		});

		let spec = spec.arg(self.attrs[0].field(true));
		vec![
			self.attrs[1..]
				.iter()
				.fold(spec, |spec, attr| spec.arg(attr.field(false))),
		]
	}

	/// The typed handle, for
	/// [`TypedExtension::handle`](canvist_core::TypedExtension::handle).
	#[must_use]
	pub fn handle(&'static self, registration: Registration) -> MediaHandle {
		MediaHandle {
			block: self,
			_registration: registration,
		}
	}
}
