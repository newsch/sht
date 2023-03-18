use std::iter;

use serde::{Deserialize, Serialize};
use tui::{
	buffer::Buffer,
	layout::Rect,
	style::Style,
	text::Text,
	widgets::{BorderType, StatefulWidget, Widget},
};

use crate::{styles, Rect as MyRect, XY};

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
	column_border: BorderType,
	even_row_style: Style,
	odd_row_style: Style,
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
			style: styles::grid(),
			// TODO: own this, use index-based method or expose default?
			widths: &[],
			column_border: BorderType::Plain,
			column_spacing: 1,
			even_row_style: Style::default(),
			odd_row_style: Style::default(),
			// odd_row_style: Style::default().bg(Color::Black).fg(Color::White),
			// odd_row_style: Style::default().add_modifier(Modifier::UNDERLINED),
			highlight_style: styles::selected(),
			rows,
		}
	}

	pub fn with_widths(mut self, widths: &'a [u16]) -> Self {
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
		let mut start = offset;
		let mut end = offset;
		let mut height = 0;
		loop {
			if height + row_height > max_height {
				break;
			}
			height += row_height;
			end += 1;
		}

		let Some(selected) = selected else {
			return (start, end);
		};

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

	fn cell_widths<'s>(&'s self) -> impl Iterator<Item = u16> + 's {
		self.widths
			.iter()
			.map(ToOwned::to_owned)
			.chain(iter::repeat(DEFAULT_WIDTH))
	}

	fn col_widths<'s>(&'s self) -> impl Iterator<Item = u16> + 's {
		self.cell_widths().map(|w| w + self.column_spacing)
	}

	fn cell_width_at(&self, col: usize) -> u16 {
		self.widths
			.get(col)
			.map(ToOwned::to_owned)
			.unwrap_or(DEFAULT_WIDTH)
	}

	fn col_width_at(&self, col: usize) -> u16 {
		self.widths.get(col).unwrap_or(&DEFAULT_WIDTH) + self.column_spacing
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
		let mut start = offset;
		let mut end = offset;
		let mut width = 0;

		for col_width in self.col_widths().skip(offset) {
			width += col_width;
			end += 1;
			if width >= max_width {
				break;
			}
		}

		let Some(selected) = selected else {
			trace!("No selection; using {:?}", (start, end));
			return (start, end);
		};

		// bring selection into view (changing offset)
		if selected >= end - 1 {
			while selected >= end {
				trace!("Correcting overshot selection");
				// add additional columns
				end += 1;
				width += self.col_width_at(end - 1);
			}
			assert!(selected == end - 1); // selection is at right side
			trace!("Bringing final column into view");
			// make sure entire selected column is in view
			while width > max_width {
				trace!("Shifting start right");
				width -= self.col_width_at(start);
				start += 1;
			}
			// include any (maybe partial) right of selected
			while width < max_width {
				trace!("Shifting end right");
				width += self.col_width_at(end);
				end += 1;
			}
			assert!(width >= max_width);
		} else if selected < start {
			while selected < start {
				trace!("Correcting undershot selection");
				start -= 1;
				width += self.col_width_at(start);
			}
			assert!(selected == start);
			while end > 0 && width.saturating_sub(self.col_width_at(end - 1)) > max_width {
				trace!("Shifting end left");
				width -= self.col_width_at(end - 1);
				end -= 1;
			}
		}
		trace!("Using {:?}", (start, end));
		assert!(selected >= start && selected < end);
		(start, end)
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TableState {
	offset: XY<usize>,
	selected: Option<XY<usize>>,
	selected_area: Option<MyRect>,
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

	/// Retrieve the location and area of the selected cell drawn in the last render
	pub fn selected_area(&self) -> Option<Rect> {
		self.selected_area.map(Into::into)
	}
}

impl<'a> StatefulWidget for Table<'a> {
	type State = TableState;

	fn render(self, area: tui::layout::Rect, buf: &mut Buffer, state: &mut Self::State) {
		if let Some(r) = self.rows.first() {
			let width = r.len();
			assert!(self.widths.len() == width);
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

		for row_t in row_start..row_end {
			let row_height = 1; // TODO
			let (row, col) = (area.top() + current_height, area.left());
			current_height += row_height;
			let table_row_area = Rect {
				x: col,
				y: row,
				width: area.width,
				height: row_height,
			};

			let row_style = if row_t % 2 == 0 {
				self.even_row_style
			} else {
				self.odd_row_style
			};
			buf.set_style(table_row_area, row_style);

			let mut col = col;

			for (col_t, width) in self
				.cell_widths()
				.enumerate()
				.skip(col_start)
				.take(col_end - col_start)
			{
				let mut cell_area = Rect {
					x: col,
					y: row,
					width,
					height: row_height,
				};
				// draw column border
				let column_x = cell_area.right();
				if column_x < table_row_area.right() {
					let x = column_x;
					for y in cell_area.y..cell_area.bottom() {
						buf.get_mut(x, y)
							.set_symbol(BorderType::line_symbols(self.column_border).vertical);
					}
				}
				cell_area = cell_area.intersection(table_row_area);
				if let Some(cell) = self.rows.get(row_t).and_then(|r| r.get(col_t)) {
					render_cell(buf, cell, cell_area);
				}
				let is_selected = state
					.selected
					.map(|s| s == XY { x: col_t, y: row_t })
					.unwrap_or_default();
				if is_selected {
					buf.set_style(cell_area, self.highlight_style);
					state.selected_area = Some(cell_area.into());
				}
				col += width + self.column_spacing;
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
