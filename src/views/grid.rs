use std::cmp::max;

use tui::{
	layout::Constraint,
	style::{Modifier, Style},
	widgets::Widget,
};

use crate::{Grid, XY};

use super::{Cell, Row, Table};

pub struct GridView<'g> {
	grid: &'g Grid,
	selection: XY<usize>,
}

impl<'g> GridView<'g> {
	pub fn new(grid: &'g Grid, selection: XY<usize>) -> Self {
		Self { grid, selection }
	}
}

impl<'g> Widget for GridView<'g> {
	fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
		let table = Table::new(self.grid.cells.iter().enumerate().map(|(y, row)| {
			let row = row.clone();
			if y == self.selection.y {
				let mut row: Vec<_> = row.into_iter().map(Cell::from).collect();
				row[self.selection.x] = row[self.selection.x]
					.clone()
					.style(Style::default().add_modifier(Modifier::BOLD));
				Row::new(row)
			} else {
				Row::new(row)
			}
		}));
		// highlight selected

		// use longest width
		let constraints = self
			.grid
			.cells
			.iter()
			.fold(vec![0; self.grid.cells.len()], |mut len, row| {
				for (i, cell) in row.iter().enumerate() {
					len[i] = max(len[i], cell.len());
				}
				len
			})
			.into_iter()
			.map(|l| l.try_into().expect("assume cell width less that u16 max"))
			.map(|l| max(l, 16))
			.map(Constraint::Length)
			.collect::<Vec<_>>();

		let table = table.widths(&constraints);

		Widget::render(table, area, buf);
	}
}
