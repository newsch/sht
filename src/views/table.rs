use tui::{
	buffer::Buffer,
	layout::Rect,
	style::{Modifier, Style},
	text::Text,
	widgets::{StatefulWidget, Widget},
};

use crate::XY;

const DEFAULT_WIDTH: u16 = 12;

/// A widget to display data in formatted columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table<'a> {
	/// Base style for the widget
	style: Style,
	/// Width constraints for each column
	// TODO: reduced constant, full sizes
	widths: &'a [u16],
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
			// TODO: own this, use index-based method or expose default?
			widths: &[],
			column_spacing: 1,
			highlight_style: Style::default().add_modifier(Modifier::REVERSED),
			rows,
		}
	}

	pub fn column_spacing(mut self, spacing: u16) -> Self {
		self.column_spacing = spacing;
		self
	}

	pub fn widths(mut self, widths: &'a [u16]) -> Self {
		self.widths = widths.as_ref();
		self
	}
}

impl<'a> Table<'a> {
	/// [start, end) indices of visible rows
	fn get_row_bounds(
		&self,
		selected: Option<usize>,
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

		let selected = selected.unwrap_or_default().min(self.rows.len() - 1);
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

	/// [start, end) indices of visible cols.
	///
	/// The final column may only be partially visible.
	fn get_col_bounds(
		&self,
		selected: Option<usize>,
		offset: usize,
		max_width: u16,
	) -> (usize, usize) {
		let offset = offset.min(self.widths.len().saturating_sub(1));
		let mut start = offset;
		let mut end = offset;
		let mut width = 0;

		for col_width in self.widths.iter().skip(offset) {
			width += col_width;
			width += self.column_spacing;
			end += 1;
			if width >= max_width {
				break;
			}
		}

		let Some(selected) = selected else {
			return (start, end);
		};

		// bring selection into view (changing offset)
		if selected >= end {
			while selected >= end {
				trace!("Correcting overshot selection");
				// add additional columns
				end += 1;
				width += self.widths[end - 1];
				width += self.column_spacing;
			}
			if selected == end - 1 {
				// make sure entire final column is in view
				while width > max_width {
					width -= self.widths[start];
					width -= self.column_spacing;
					start += 1;
				}
				if width < max_width {
					width += self.widths[start];
					width += self.column_spacing;
					end += 1;
				}
			} else {
				// get to end overlapping
				while width - self.widths[end - 1] > max_width {
					// remove trailing ones
					width -= self.widths[start];
					width -= self.column_spacing;
					start += 1;
				}
			}
		} else if selected < start {
			while selected < start {
				trace!("Correcting undershot selection");
				start -= 1;
				width += self.widths[start];
				width += self.column_spacing;
			}
			while width > max_width {
				end -= 1;
				width -= self.widths[end];
				width -= self.column_spacing;
			}
		}
		assert!(selected >= start && selected < end);
		(start, end)
	}
}

#[derive(Debug, Clone, Default)]
pub struct TableState {
	offset: XY<usize>,
	selected: Option<XY<usize>>,
}

impl TableState {
	pub fn selected(&self) -> Option<XY<usize>> {
		self.selected
	}

	pub fn select(&mut self, index: Option<XY<usize>>) {
		self.selected = index;
		if index.is_none() {
			self.offset = Default::default();
		}
	}
}

impl<'a> StatefulWidget for Table<'a> {
	type State = TableState;

	fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
		if let Some(r) = self.rows.first() {
			let width = r.len();
			let height = self.rows.len();
			assert!(self.widths.len() == width);
			assert!(state.offset.x < width);
			assert!(state.offset.y < height);
			assert!(state.offset.x < width);
			assert!(state.offset.y < height);
		}
		// TODO: handle constraining/reseting
		if area.area() == 0 {
			return;
		}

		buf.set_style(area, self.style);

		// Draw rows
		if self.rows.is_empty() {
			return;
		}

		let mut current_height = 0;

		let (row_start, row_end) =
			self.get_row_bounds(state.selected.map(|s| s.y), state.offset.y, area.height);
		state.offset.y = row_start;
		let (col_start, col_end) =
			self.get_col_bounds(state.selected.map(|s| s.x), state.offset.x, area.width);
		state.offset.x = col_start;
		for (row_t, table_row) in self
			.rows
			.iter()
			.enumerate()
			.skip(row_start)
			.take(row_end - row_start)
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

			for (col_t, (width, cell)) in self
				.widths
				.iter()
				.zip(table_row.iter())
				.enumerate()
				.skip(col_start)
				.take(col_end - col_start)
			{
				let mut cell_area = Rect {
					x: col,
					y: row,
					width: *width,
					height: row_height,
				};
				cell_area = cell_area.intersection(table_row_area);
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
