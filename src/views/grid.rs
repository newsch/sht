use std::cmp::max;

use tui::{
	buffer::Buffer,
	layout::{Constraint, Rect},
	style::{Modifier, Style},
	widgets::{StatefulWidget, Widget},
};

use crate::{Grid, XY};

use super::{Row, Table, TableState};

#[derive(Default)]
pub struct GridState(TableState);

impl GridState {
	pub fn select(&mut self, s: XY<usize>) {
		self.0.select(Some(s));
	}
}

pub struct GridView<'g> {
	grid: &'g Grid,
	selection: XY<usize>,
}

impl<'g> GridView<'g> {
	pub fn new(grid: &'g Grid, selection: XY<usize>) -> Self {
		Self { grid, selection }
	}
}

impl<'g> StatefulWidget for GridView<'g> {
	type State = GridState;

	fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
		let table = Table::new(self.grid.cells.iter().map(|row| {
			let row = row.clone();
			Row::new(row)
		}));
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

		StatefulWidget::render(table, area, buf, &mut state.0);
	}
}

impl<'g> Widget for GridView<'g> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let mut state = GridState::default();
		StatefulWidget::render(self, area, buf, &mut state);
	}
}
