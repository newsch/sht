use tui::{
	buffer::Buffer,
	layout::{Constraint, Direction, Layout, Rect},
	style::{Modifier, Style},
	text::Text,
	widgets::{StatefulWidget, Widget},
};

use crate::XY;

/// A widget to display data in formatted columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table<'a> {
	/// Base style for the widget
	style: Style,
	/// Width constraints for each column
	widths: &'a [Constraint],
	// /// Height constraints for each row
	// TODO
	// heights: &'a [Constraint],
	/// Space between each column
	column_spacing: u16,
	/// Style used to render the selected row
	highlight_style: Style,
	// /// Optional header
	// TODO: Frozen headers/columns
	// header: Option<usize>,
	/// Data to display in each row
	rows: &'a Vec<Vec<String>>,
}

impl<'a> Table<'a> {
	pub fn new(rows: &'a Vec<Vec<String>>) -> Self {
		Self {
			style: Style::default(),
			widths: &[],
			column_spacing: 1,
			highlight_style: Style::default().add_modifier(Modifier::REVERSED),
			rows,
		}
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
		let row_height = 1; // TODO: proper row heights
		let offset = offset.min(self.rows.len().saturating_sub(1));
		let mut start = offset;
		let mut end = offset;
		let mut height = 0;
		for _item in self.rows.iter().skip(offset) {
			if height + row_height > max_height {
				break;
			}
			height += row_height;
			end += 1;
		}

		let selected = selected.unwrap_or_default().y.min(self.rows.len() - 1);
		while selected >= end {
			height = height.saturating_add(row_height);
			end += 1;
			while height > max_height {
				height = height.saturating_sub(row_height);
				start += 1;
			}
		}
		while selected < start {
			start -= 1;
			height = height.saturating_add(row_height);
			while height > max_height {
				end -= 1;
				height = height.saturating_sub(row_height);
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

	fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
		if area.area() == 0 {
			return;
		}

		buf.set_style(area, self.style);

		let columns_widths = self.get_columns_widths(area.width);
		let mut current_height = 0;
		let rows_height = area.height;

		// Draw rows
		if self.rows.is_empty() {
			return;
		}
		let (start, end) = self.get_row_bounds(state.selected, state.offset, rows_height);
		state.offset = start;
		for (row_t, table_row) in self
			.rows
			.iter()
			.enumerate()
			.skip(state.offset)
			.take(end - start)
		{
			let row_height = 1; // TODO
			let (row, col) = (area.top() + current_height, area.left());
			current_height += row_height;
			let table_row_area = Rect {
				x: col,
				y: row,
				width: area.width,
				height: row_height,
			};
			buf.set_style(table_row_area, self.style);
			let mut col = col;
			for (col_t, (width, cell)) in columns_widths.iter().zip(table_row.iter()).enumerate() {
				let cell_area = Rect {
					x: col,
					y: row,
					width: *width,
					height: row_height,
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

fn render_cell(buf: &mut Buffer, cell: &str, area: Rect) {
	let text = Text::raw(cell);
	for (i, spans) in text.lines.iter().enumerate() {
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
		Table::new(&vec![]).widths(&[Constraint::Percentage(110)]);
	}
}
