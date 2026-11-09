#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_extensions`
//!
//! The canvist StarterKit: the extensions TipTap's StarterKit ships, built
//! on [`canvist_core`] and named the way TipTap names them, so documents
//! move between the two as ProseMirror JSON without conversion.
//!
//! Each extension is its own crate, `canvist_extension_<name>` (such as
//! `canvist_extension_heading`), and this crate combines them: each sits
//! behind a Cargo feature of the same name, which enables the crate and
//! re-exports its types here. The `starter-kit` feature (on by default)
//! enables the StarterKit's. An app that only needs bold paragraphs
//! compiles only bold paragraphs, and an app that wants no combined crate
//! at all can depend on the extension crates directly.
//!
//! | Extension          | Feature           | Adds                                    |
//! | ------------------ | ----------------- | --------------------------------------- |
//! | [`Document`]       | always            | The root                                |
//! | [`Paragraph`]      | always            | `paragraph`, `Mod-Alt-0`                |
//! | [`Heading`]        | `heading`         | `heading`, `#` to `######`, `Mod-Alt-n` |
//! | [`Blockquote`]     | `blockquote`      | `blockquote`, `> `, `Mod-Shift-b`       |
//! | [`CodeBlock`]      | `code-block`      | `codeBlock`, ```` ``` ````, `Mod-Alt-c` |
//! | [`HardBreak`]      | `hard-break`      | `hardBreak`, `Shift-Enter`              |
//! | [`HorizontalRule`] | `horizontal-rule` | `horizontalRule`, `---`                 |
//! | [`BulletList`]     | `bullet-list`     | `bulletList`, `- `, `Mod-Shift-8`       |
//! | [`OrderedList`]    | `ordered-list`    | `orderedList`, `1. `, `Mod-Shift-7`     |
//! | [`ListItem`]       | `list-item`       | `listItem`, Enter/Tab/Shift-Tab         |
//! | [`TaskList`]       | `task-list`       | `taskList`, `taskItem`, `[ ] `          |
//! | [`Bold`]           | `bold`            | `bold`, `**text**`, `Mod-b`             |
//! | [`Italic`]         | `italic`          | `italic`, `*text*`, `Mod-i`             |
//! | [`Underline`]      | `underline`       | `underline`, `Mod-u`                    |
//! | [`Strike`]         | `strike`          | `strike`, `~~text~~`, `Mod-Shift-s`     |
//! | [`Code`]           | `code`            | `code`, `` `text` ``, `Mod-e`           |
//! | [`Link`]           | `link`            | `link`, autolink, link on paste         |
//! | [`History`]        | `history`         | `Mod-z`, `Mod-Shift-z`, `Mod-y`         |
//! | [`TrailingNode`]   | `trailing-node`   | A trailing paragraph                    |
//!
//! Beyond the StarterKit, each behind its own feature (`full` enables
//! them all, and [`extras`] lists them):
//!
//! | Extension           | Feature             | Adds                                     |
//! | ------------------- | ------------------- | ---------------------------------------- |
//! | [`Subscript`]       | `subscript`         | `subscript`, `Mod-,`                     |
//! | [`Superscript`]     | `superscript`       | `superscript`, `Mod-.`                   |
//! | [`Highlight`]       | `highlight`         | `highlight`, `==text==`, `Mod-Shift-h`   |
//! | [`TextStyle`]       | `text-style`        | `textStyle`, the mark styles live on     |
//! | [`Color`]           | `color`             | `textStyle.color`                        |
//! | [`BackgroundColor`] | `background-color`  | `textStyle.backgroundColor`              |
//! | [`FontFamily`]      | `font-family`       | `textStyle.fontFamily`                   |
//! | [`FontSize`]        | `font-size`         | `textStyle.fontSize`                     |
//! | [`LineHeight`]      | `line-height`       | `textStyle.lineHeight`                   |
//! | [`TextAlign`]       | `text-align`        | `textAlign`, `Mod-Shift-l`/`e`/`r`/`j`   |
//! | [`Typography`]      | `typography`        | Smart quotes, dashes, arrows, fractions  |
//! | [`CharacterCount`]  | `character-count`   | Character and word counts, a limit       |
//! | [`Placeholder`]     | `placeholder`       | Hint text in empty blocks                |
//! | [`Mention`]         | `mention`           | `mention`, `@` suggestions               |
//! | [`Hashtag`]         | `hashtag`           | `hashtag`, `#tag` as you type            |
//! | [`Emoji`]           | `emoji`             | `emoji`, `:smile:`, `:` suggestions      |
//! | [`Image`]           | `image`             | `image`, `![alt](src)`                   |
//! | [`Youtube`]         | `youtube`           | `youtube`                                |
//! | [`Twitch`]          | `twitch`            | `twitch`                                 |
//! | [`Audio`]           | `audio`             | `audio`                                  |
//! | [`File`]            | `file`              | `file`, an attachment card               |
//! | [`Details`]         | `details`           | `details`, a summary that opens          |
//! | [`Callout`]         | `callout`           | `callout`, `:::warning `                 |
//! | [`Mathematics`]     | `mathematics`       | `inlineMath`, `blockMath`, `$$x$$`       |
//! | [`Table`]           | `table`             | `table`, rows, cells, and header cells   |
//! | [`FindAndReplace`]  | `find-and-replace`  | search, step, and replace                |
//! | [`TableOfContents`] | `table-of-contents` | the headings as an outline               |
//! | [`Comments`]        | `comments`          | `comment`, thread ids on text            |
//! | [`DragHandle`]      | `drag-handle`       | move blocks, Alt-↑ and Alt-↓             |
//! | [`Ink`]             | `ink`               | `ink`, freehand strokes the canvas draws |
//! | [`Kinetic`]         | `kinetic`           | `kinetic`, text that moves               |
//! | [`Conditional`]     | `conditional`       | `conditional`, text some versions keep   |
//! | [`ClearFormatting`] | `clear-formatting`  | marks off, blocks to paragraphs, `Mod-\` |
//!
//! `docs/src/extensions/parity.md` compares the whole set with Remirror and
//! TipTap.
//!
//! # Example
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_core::Editor;
//! use canvist_core::Env;
//! use canvist_extensions::StarterKit;
//!
//! let mut editor = Editor::builder()
//! 	.extensions(StarterKit::default().extensions())
//! 	.build()?;
//! let mut env = Env::headless(Duration::ZERO);
//!
//! for text in ["#", " ", "H", "i"] {
//! 	editor.insert_text(text, &mut env)?;
//! }
//!
//! let heading = editor.schema().block_type("heading");
//! let first = editor.state().doc().first_text_block().and_then(|id| editor.state().doc().block(id));
//! assert_eq!(first.map(|block| block.kind()), heading);
//! # Ok::<(), canvist_core::EditorError>(())
//! ```

#[cfg(feature = "clipboard")]
pub use canvist_extension_clipboard as clipboard;
#[cfg(feature = "emoji")]
pub use canvist_extension_emoji as emoji;
#[cfg(feature = "extras")]
mod extras;
mod starter_kit;

#[cfg(any(
	feature = "bold",
	feature = "italic",
	feature = "underline",
	feature = "strike",
	feature = "code",
	feature = "subscript",
	feature = "superscript"
))]
pub use canvist_core::MarkHandle;
#[cfg(feature = "audio")]
pub use canvist_extension_audio::Audio;
#[cfg(feature = "blockquote")]
pub use canvist_extension_blockquote::Blockquote;
#[cfg(feature = "blockquote")]
pub use canvist_extension_blockquote::BlockquoteHandle;
#[cfg(feature = "bold")]
pub use canvist_extension_bold::Bold;
#[cfg(feature = "callout")]
pub use canvist_extension_callout::Callout;
#[cfg(feature = "callout")]
pub use canvist_extension_callout::CalloutHandle;
#[cfg(feature = "callout")]
pub use canvist_extension_callout::CalloutType;
#[cfg(feature = "character-count")]
pub use canvist_extension_character_count::CharacterCount;
#[cfg(feature = "character-count")]
pub use canvist_extension_character_count::CharacterCountHandle;
#[cfg(feature = "clear-formatting")]
pub use canvist_extension_clear_formatting::ClearFormatting;
#[cfg(feature = "clear-formatting")]
pub use canvist_extension_clear_formatting::ClearFormattingHandle;
#[cfg(feature = "code")]
pub use canvist_extension_code::Code;
#[cfg(feature = "code-block")]
pub use canvist_extension_code_block::CodeBlock;
#[cfg(feature = "code-block")]
pub use canvist_extension_code_block::CodeBlockHandle;
#[cfg(feature = "comments")]
pub use canvist_extension_comments::Comments;
#[cfg(feature = "comments")]
pub use canvist_extension_comments::CommentsHandle;
#[cfg(feature = "conditional")]
pub use canvist_extension_conditional::Conditional;
#[cfg(feature = "conditional")]
pub use canvist_extension_conditional::ConditionalHandle;
#[cfg(feature = "details")]
pub use canvist_extension_details::Details;
#[cfg(feature = "details")]
pub use canvist_extension_details::DetailsHandle;
pub use canvist_extension_document::Document;
#[cfg(feature = "drag-handle")]
pub use canvist_extension_drag_handle::DragHandle;
#[cfg(feature = "drag-handle")]
pub use canvist_extension_drag_handle::DragHandleHandle;
#[cfg(feature = "emoji")]
pub use canvist_extension_emoji::Emoji;
#[cfg(feature = "emoji")]
pub use canvist_extension_emoji::EmojiHandle;
#[cfg(feature = "file")]
pub use canvist_extension_file::File;
#[cfg(feature = "find-and-replace")]
pub use canvist_extension_find_and_replace::FindAndReplace;
#[cfg(feature = "find-and-replace")]
pub use canvist_extension_find_and_replace::FindAndReplaceHandle;
#[cfg(feature = "find-and-replace")]
pub use canvist_extension_find_and_replace::SearchMatch;
#[cfg(feature = "hard-break")]
pub use canvist_extension_hard_break::HardBreak;
#[cfg(feature = "hard-break")]
pub use canvist_extension_hard_break::HardBreakHandle;
#[cfg(feature = "hashtag")]
pub use canvist_extension_hashtag::Hashtag;
#[cfg(feature = "hashtag")]
pub use canvist_extension_hashtag::HashtagHandle;
#[cfg(feature = "heading")]
pub use canvist_extension_heading::Heading;
#[cfg(feature = "heading")]
pub use canvist_extension_heading::HeadingHandle;
#[cfg(feature = "heading")]
pub use canvist_extension_heading::HeadingLevel;
#[cfg(feature = "highlight")]
pub use canvist_extension_highlight::Highlight;
#[cfg(feature = "highlight")]
pub use canvist_extension_highlight::HighlightHandle;
#[cfg(feature = "history")]
pub use canvist_extension_history::History;
#[cfg(feature = "history")]
pub use canvist_extension_history::HistoryHandle;
#[cfg(feature = "horizontal-rule")]
pub use canvist_extension_horizontal_rule::HorizontalRule;
#[cfg(feature = "horizontal-rule")]
pub use canvist_extension_horizontal_rule::HorizontalRuleHandle;
#[cfg(feature = "image")]
pub use canvist_extension_image::Image;
#[cfg(feature = "ink")]
pub use canvist_extension_ink::INK_WIDTH;
#[cfg(feature = "ink")]
pub use canvist_extension_ink::Ink;
#[cfg(feature = "ink")]
pub use canvist_extension_ink::InkHandle;
#[cfg(feature = "ink")]
pub use canvist_extension_ink::InkStroke;
#[cfg(feature = "italic")]
pub use canvist_extension_italic::Italic;
#[cfg(feature = "kinetic")]
pub use canvist_extension_kinetic::Kinetic;
#[cfg(feature = "kinetic")]
pub use canvist_extension_kinetic::KineticEffect;
#[cfg(feature = "kinetic")]
pub use canvist_extension_kinetic::KineticHandle;
#[cfg(feature = "link")]
pub use canvist_extension_link::Link;
#[cfg(feature = "link")]
pub use canvist_extension_link::LinkHandle;
#[cfg(feature = "bullet-list")]
pub use canvist_extension_list::BulletList;
#[cfg(feature = "bullet-list")]
pub use canvist_extension_list::BulletListHandle;
#[cfg(feature = "list-item")]
pub use canvist_extension_list::ListItem;
#[cfg(feature = "list-item")]
pub use canvist_extension_list::ListItemHandle;
#[cfg(feature = "ordered-list")]
pub use canvist_extension_list::OrderedList;
#[cfg(feature = "ordered-list")]
pub use canvist_extension_list::OrderedListHandle;
#[cfg(feature = "task-list")]
pub use canvist_extension_list::TaskList;
#[cfg(feature = "task-list")]
pub use canvist_extension_list::TaskListHandle;
#[cfg(feature = "mathematics")]
pub use canvist_extension_mathematics::Mathematics;
#[cfg(feature = "mathematics")]
pub use canvist_extension_mathematics::MathematicsHandle;
#[cfg(any(
	feature = "audio",
	feature = "file",
	feature = "image",
	feature = "twitch",
	feature = "youtube"
))]
pub use canvist_extension_media::MediaHandle;
#[cfg(feature = "mention")]
pub use canvist_extension_mention::Mention;
#[cfg(feature = "mention")]
pub use canvist_extension_mention::MentionHandle;
#[cfg(feature = "mention")]
pub use canvist_extension_mention::MentionTarget;
pub use canvist_extension_paragraph::Paragraph;
pub use canvist_extension_paragraph::ParagraphHandle;
#[cfg(feature = "placeholder")]
pub use canvist_extension_placeholder::Placeholder;
#[cfg(feature = "placeholder")]
pub use canvist_extension_placeholder::PlaceholderHandle;
#[cfg(feature = "strike")]
pub use canvist_extension_strike::Strike;
#[cfg(feature = "subscript")]
pub use canvist_extension_subscript::Subscript;
#[cfg(feature = "superscript")]
pub use canvist_extension_superscript::Superscript;
#[cfg(feature = "table")]
pub use canvist_extension_table::HeaderPart;
#[cfg(feature = "table")]
pub use canvist_extension_table::Table;
#[cfg(feature = "table")]
pub use canvist_extension_table::TableCell;
#[cfg(feature = "table")]
pub use canvist_extension_table::TableHandle;
#[cfg(feature = "table-of-contents")]
pub use canvist_extension_table_of_contents::TableOfContents;
#[cfg(feature = "table-of-contents")]
pub use canvist_extension_table_of_contents::TableOfContentsHandle;
#[cfg(feature = "table-of-contents")]
pub use canvist_extension_table_of_contents::TocEntry;
#[cfg(feature = "text-align")]
pub use canvist_extension_text_align::Alignment;
#[cfg(feature = "text-align")]
pub use canvist_extension_text_align::TextAlign;
#[cfg(feature = "text-align")]
pub use canvist_extension_text_align::TextAlignHandle;
#[cfg(feature = "background-color")]
pub use canvist_extension_text_style::BackgroundColor;
#[cfg(feature = "color")]
pub use canvist_extension_text_style::Color;
#[cfg(feature = "font-family")]
pub use canvist_extension_text_style::FontFamily;
#[cfg(feature = "font-size")]
pub use canvist_extension_text_style::FontSize;
#[cfg(feature = "line-height")]
pub use canvist_extension_text_style::LineHeight;
#[cfg(feature = "text-style")]
pub use canvist_extension_text_style::StyleAttrHandle;
#[cfg(feature = "text-style")]
pub use canvist_extension_text_style::TextStyle;
#[cfg(feature = "text-style")]
pub use canvist_extension_text_style::TextStyleHandle;
#[cfg(feature = "trailing-node")]
pub use canvist_extension_trailing_node::TrailingNode;
#[cfg(feature = "twitch")]
pub use canvist_extension_twitch::Twitch;
#[cfg(feature = "typography")]
pub use canvist_extension_typography::Typography;
#[cfg(feature = "typography")]
pub use canvist_extension_typography::TypographyHandle;
#[cfg(feature = "underline")]
pub use canvist_extension_underline::Underline;
#[cfg(feature = "youtube")]
pub use canvist_extension_youtube::Youtube;
#[cfg(feature = "extras")]
pub use extras::extras;
pub use starter_kit::StarterKit;
pub use starter_kit::StarterKitHandles;
