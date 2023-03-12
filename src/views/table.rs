use tui::{
	buffer::Buffer,
	layout::{Constraint, Direction, Layout, Rect},
	style::{Modifier, Style},
	text::Text,
	widgets::{Block, StatefulWidget, Widget},
};

use crate::XY;

/// A [`Cell`] contains the [`Text`] to be displayed in a [`Row`] of a [`Table`].
///
/// It can be created from anything that can be converted to a [`Text`].
/// ```rust
/// # use tui::widgets::Cell;
/// # use tui::style::{Style, Modifier};
/// # use tui::text::{Span, Spans, Text};
/// # use std::borrow::Cow;
/// Cell::from("simple string");
///
/// Cell::from(Span::from("span"));
///
/// Cell::from(Spans::from(vec![
///     Span::raw("a vec of "),
///     Span::styled("spans", Style::default().add_modifier(Modifier::BOLD))
/// ]));
///
/// Cell::from(Text::from("a text"));
///
/// Cell::from(Text::from(Cow::Borrowed("hello")));
/// ```
///
/// You can apply a [`Style`] on the entire [`Cell`] using [`Cell::style`] or rely on the styling
/// capabilities of [`Text`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cell<'a> {
	content: Text<'a>,
	style: Style,
}

impl<'a> Cell<'a> {
	/// Set the `Style` of this cell.
	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}
}

impl<'a, T> From<T> for Cell<'a>
where
	T: Into<Text<'a>>,
{
	fn from(content: T) -> Cell<'a> {
		Cell {
			content: content.into(),
			style: Style::default(),
		}
	}
}

/// Holds data to be displayed in a [`Table`] widget.
///
/// A [`Row`] is a collection of cells. It can be created from simple strings:
/// ```rust
/// # use tui::widgets::Row;
/// Row::new(vec!["Cell1", "Cell2", "Cell3"]);
/// ```
///
/// But if you need a bit more control over individual cells, you can explicity create [`Cell`]s:
/// ```rust
/// # use tui::widgets::{Row, Cell};
/// # use tui::style::{Style, Color};
/// Row::new(vec![
///     Cell::from("Cell1"),
///     Cell::from("Cell2").style(Style::default().fg(Color::Yellow)),
/// ]);
/// ```
///
/// You can also construct a row from any type that can be converted into [`Text`]:
/// ```rust
/// # use std::borrow::Cow;
/// # use tui::widgets::Row;
/// Row::new(vec![
///     Cow::Borrowed("hello"),
///     Cow::Owned("world".to_uppercase()),
/// ]);
/// ```
///
/// By default, a row has a height of 1 but you can change this using [`Row::height`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row<'a> {
	cells: Vec<Cell<'a>>,
	height: u16,
	style: Style,
	bottom_margin: u16,
}

impl<'a> Row<'a> {
	/// Creates a new [`Row`] from an iterator where items can be converted to a [`Cell`].
	pub fn new<T>(cells: T) -> Self
	where
		T: IntoIterator,
		T::Item: Into<Cell<'a>>,
	{
		Self {
			height: 1,
			cells: cells.into_iter().map(|c| c.into()).collect(),
			style: Style::default(),
			bottom_margin: 0,
		}
	}

	/// Set the fixed height of the [`Row`]. Any [`Cell`] whose content has more lines than this
	/// height will see its content truncated.
	pub fn height(mut self, height: u16) -> Self {
		self.height = height;
		self
	}

	/// Set the [`Style`] of the entire row. This [`Style`] can be overriden by the [`Style`] of a
	/// any individual [`Cell`] or event by their [`Text`] content.
	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Set the bottom margin. By default, the bottom margin is `0`.
	pub fn bottom_margin(mut self, margin: u16) -> Self {
		self.bottom_margin = margin;
		self
	}

	/// Returns the total height of the row.
	fn total_height(&self) -> u16 {
		self.height.saturating_add(self.bottom_margin)
	}
}

/// A widget to display data in formatted columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table<'a> {
	/// A block to wrap the widget in
	block: Option<Block<'a>>,
	/// Base style for the widget
	style: Style,
	/// Width constraints for each column
	widths: &'a [Constraint],
	/// Space between each column
	column_spacing: u16,
	/// Style used to render the selected row
	highlight_style: Style,
	/// Optional header
	header: Option<Row<'a>>,
	/// Data to display in each row
	rows: Vec<Row<'a>>,
}

impl<'a> Table<'a> {
	pub fn new<T>(rows: T) -> Self
	where
		T: IntoIterator<Item = Row<'a>>,
	{
		Self {
			block: None,
			style: Style::default(),
			widths: &[],
			column_spacing: 1,
			highlight_style: Style::default().add_modifier(Modifier::REVERSED),
			header: None,
			rows: rows.into_iter().collect(),
		}
	}

	pub fn block(mut self, block: Block<'a>) -> Self {
		self.block = Some(block);
		self
	}

	pub fn header(mut self, header: Row<'a>) -> Self {
		self.header = Some(header);
		self
	}

	pub fn widths(mut self, widths: &'a [Constraint]) -> Self {
		let between_0_and_100 = |&w| match w {
			Constraint::Percentage(p) => p <= 100,
			_ => true,
		};
		assert!(
			widths.iter().all(between_0_and_100),
			"Percentages should be between 0 and 100 inclusively."
		);
		self.widths = widths;
		self
	}

	pub fn column_spacing(mut self, spacing: u16) -> Self {
		self.column_spacing = spacing;
		self
	}

	fn get_columns_widths(&self, max_width: u16) -> Vec<u16> {
		let mut constraints = Vec::with_capacity(self.widths.len() * 2 + 1);
		for constraint in self.widths {
			constraints.push(*constraint);
			constraints.push(Constraint::Length(self.column_spacing));
		}
		if !self.widths.is_empty() {
			constraints.pop();
		}
		let chunks = Layout::default()
			.direction(Direction::Horizontal)
			.constraints(constraints)
			.split(Rect {
				x: 0,
				y: 0,
				width: max_width,
				height: 1,
			});
		chunks.iter().step_by(2).map(|c| c.width).collect()
	}

	fn get_row_bounds(
		&self,
		selected: Option<XY<usize>>,
		offset: usize,
		max_height: u16,
	) -> (usize, usize) {
		let offset = offset.min(self.rows.len().saturating_sub(1));
		let mut start = offset;
		let mut end = offset;
		let mut height = 0;
		for item in self.rows.iter().skip(offset) {
			if height + item.height > max_height {
				break;
			}
			height += item.total_height();
			end += 1;
		}

		let selected = selected.unwrap_or_default().y.min(self.rows.len() - 1);
		while selected >= end {
			height = height.saturating_add(self.rows[end].total_height());
			end += 1;
			while height > max_height {
				height = height.saturating_sub(self.rows[start].total_height());
				start += 1;
			}
		}
		while selected < start {
			start -= 1;
			height = height.saturating_add(self.rows[start].total_height());
			while height > max_height {
				end -= 1;
				height = height.saturating_sub(self.rows[end].total_height());
			}
		}
		(start, end)
	}
}

#[derive(Debug, Clone, Default)]
pub struct TableState {
	offset: usize,
	selected: Option<XY<usize>>,
}

impl TableState {
	pub fn selected(&self) -> Option<XY<usize>> {
		self.selected
	}

	pub fn select(&mut self, index: Option<XY<usize>>) {
		self.selected = index;
		if index.is_none() {
			self.offset = 0;
		}
	}
}

impl<'a> StatefulWidget for Table<'a> {
	type State = TableState;

	fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
		if area.area() == 0 {
			return;
		}
		buf.set_style(area, self.style);
		let table_area = match self.block.take() {
			Some(b) => {
				let inner_area = b.inner(area);
				b.render(area, buf);
				inner_area
			}
			None => area,
		};

		let columns_widths = self.get_columns_widths(table_area.width);
		let mut current_height = 0;
		let mut rows_height = table_area.height;

		// Draw header
		if let Some(ref header) = self.header {
			let max_header_height = table_area.height.min(header.total_height());
			buf.set_style(
				Rect {
					x: table_area.left(),
					y: table_area.top(),
					width: table_area.width,
					height: table_area.height.min(header.height),
				},
				header.style,
			);
			let mut col = table_area.left();
			for (width, cell) in columns_widths.iter().zip(header.cells.iter()) {
				render_cell(
					buf,
					cell,
					Rect {
						x: col,
						y: table_area.top(),
						width: *width,
						height: max_header_height,
					},
				);
				col += *width + self.column_spacing;
			}
			current_height += max_header_height;
			rows_height = rows_height.saturating_sub(max_header_height);
		}

		// Draw rows
		if self.rows.is_empty() {
			return;
		}
		let (start, end) = self.get_row_bounds(state.selected, state.offset, rows_height);
		state.offset = start;
		for (row_t, table_row) in self
			.rows
			.iter_mut()
			.enumerate()
			.skip(state.offset)
			.take(end - start)
		{
			let (row, col) = (table_area.top() + current_height, table_area.left());
			current_height += table_row.total_height();
			let table_row_area = Rect {
				x: col,
				y: row,
				width: table_area.width,
				height: table_row.height,
			};
			buf.set_style(table_row_area, table_row.style);
			let mut col = col;
			for (col_t, (width, cell)) in columns_widths
				.iter()
				.zip(table_row.cells.iter())
				.enumerate()
			{
				let cell_area = Rect {
					x: col,
					y: row,
					width: *width,
					height: table_row.height,
				};
				render_cell(buf, cell, cell_area);
				let is_selected = state
					.selected
					.map(|s| s == XY { x: col_t, y: row_t })
					.unwrap_or_default();
				if is_selected {
					buf.set_style(cell_area, self.highlight_style);
				}
				col += *width + self.column_spacing;
			}
		}
	}
}

fn render_cell(buf: &mut Buffer, cell: &Cell, area: Rect) {
	buf.set_style(area, cell.style);
	for (i, spans) in cell.content.lines.iter().enumerate() {
		if i as u16 >= area.height {
			break;
		}
		buf.set_spans(area.x, area.y + i as u16, spans, area.width);
	}
}

impl<'a> Widget for Table<'a> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let mut state = TableState::default();
		StatefulWidget::render(self, area, buf, &mut state);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	#[should_panic]
	fn table_invalid_percentages() {
		Table::new(vec![]).widths(&[Constraint::Percentage(110)]);
	}
}
