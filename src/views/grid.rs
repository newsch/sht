use std::cmp::max;

use tui::{
	buffer::Buffer,
	layout::Rect,
	widgets::{StatefulWidget, Widget},
};

use crate::{Grid, XY};

use super::{Table, TableState};

#[derive(Default)]
pub struct GridState(TableState);

impl GridState {
	pub fn select(&mut self, s: XY<usize>) {
		self.0.select(Some(s));
	}

	pub fn selected_area(&self) -> Option<Rect> {
		self.0.selected_area()
	}
}

pub struct GridView<'g> {
	grid: &'g Grid,
}

impl<'g> GridView<'g> {
	pub fn new(grid: &'g Grid) -> Self {
		Self { grid }
	}
}

impl<'g> StatefulWidget for GridView<'g> {
	type State = GridState;

	fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
		let table = Table::new(self.grid.cells());
		// use longest width
		let width = self
			.grid
			.cells()
			.first()
			.map(|r| r.len())
			.unwrap_or_default();
		let constraints = self
			.grid
			.cells()
			.iter()
			.fold(vec![0; width], |mut len, row| {
				for (i, cell) in row.iter().enumerate() {
					len[i] = max(len[i], cell.len());
				}
				len
			})
			.into_iter()
			.map(|l| l.try_into().expect("assume cell width less that u16 max"))
			.map(|l| max(l, 16))
			.collect::<Vec<_>>();

		let table = table.with_widths(&constraints);

		StatefulWidget::render(table, area, buf, &mut state.0);
	}
}

impl<'g> Widget for GridView<'g> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let mut state = GridState::default();
		StatefulWidget::render(self, area, buf, &mut state);
	}
}
