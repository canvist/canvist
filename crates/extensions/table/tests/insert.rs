//! Inserting tables through the typed handle.

use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_extension_document::Document;
use canvist_extension_paragraph::Paragraph;
use canvist_extension_table::Table;

/// The most rows or columns `insertTable` makes.
const MAX_SIZE: usize = 1_000;

/// Regression: a table of no rows, or rows of no cells, was inserted,
/// which no caret can enter and no command can edit.
#[test]
fn sizes_insert_table_refuses_insert_nothing() {
	for (rows, cols) in [(0, 0), (0, 3), (3, 0), (MAX_SIZE + 1, 1), (1, MAX_SIZE + 1)] {
		let mut builder = Editor::builder().extension(Document).extension(Paragraph);
		let table = builder.add(Table);
		let mut editor = builder.build().expect("the extensions fit together");
		let mut env = Env::headless(Duration::ZERO);

		let update = editor
			.run(&table.insert(rows, cols, true), &mut env)
			.expect("inserting never fails");

		assert!(update.is_none(), "{rows} by {cols}");
		assert_eq!(
			editor.state().doc().children(None).len(),
			1,
			"{rows} by {cols}"
		);
	}
}

#[test]
fn a_table_puts_the_caret_in_its_first_cell() {
	let mut builder = Editor::builder().extension(Document).extension(Paragraph);
	let table = builder.add(Table);
	let mut editor = builder.build().expect("the extensions fit together");
	let mut env = Env::headless(Duration::ZERO);

	editor
		.run(&table.insert(MAX_SIZE, 1, false), &mut env)
		.expect("inserting never fails");

	let cell = table.cell(editor.state()).expect("the caret is in a cell");
	assert_eq!((cell.row, cell.column, cell.header), (0, 0, false));
	assert_eq!((cell.rows, cell.columns), (MAX_SIZE, 1));
}
