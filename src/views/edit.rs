use std::{mem, ops::ControlFlow};

use crossterm::event::KeyCode;
use tui::{layout::Rect, widgets::StatefulWidget};

use crate::{Input, XY};

use super::Dialog;

#[derive(Default)]
pub struct EditView();

// TODO: use grapheme clusters instead...
#[derive(Debug, Clone)]
pub struct EditState {
	buffer: Vec<char>,
	/// [0, buffer.len()]
	cursor: usize,
}

impl EditState {
	pub fn from_str(s: &str) -> Self {
		let buffer: Vec<_> = s.chars().collect();
		Self {
			cursor: buffer.len(),
			buffer,
		}
	}

	/// Iterator of current chars
	pub fn iter(&self) -> impl Iterator<Item = &char> {
		self.buffer.iter()
	}

	/// Remove the character right of the cursor.
	fn pop_char_right(&mut self) {
		if self.cursor >= self.buffer.len() {
			return;
		}
		self.buffer.remove(self.cursor);
	}

	/// Remove the character left of the cursor.
	fn pop_char_left(&mut self) {
		if self.cursor <= 0 {
			return;
		}
		self.buffer.remove(self.cursor - 1);
		self.cursor -= 1;
	}

	/// Insert a character at the current position.
	fn insert_char(&mut self, c: char) {
		self.buffer.insert(self.cursor, c);
		self.cursor += 1;
	}

	fn move_left(&mut self) {
		if self.cursor <= 0 {
			return;
		}
		self.cursor -= 1;
	}

	fn move_right(&mut self) {
		if self.cursor >= self.buffer.len() {
			return;
		}
		self.cursor += 1;
	}

	fn move_beginning(&mut self) {
		self.cursor = 0;
	}

	fn move_end(&mut self) {
		self.cursor = self.buffer.len();
	}

	/// Remove the contents as a string
	pub fn take(&mut self) -> String {
		mem::take(&mut self.buffer).into_iter().collect()
	}
}

impl StatefulWidget for EditView {
	type State = EditState;

	fn render(self, area: Rect, buf: &mut tui::buffer::Buffer, state: &mut Self::State) {
		// TODO: handle overflow w/ ellipses
		let y = area.y;
		for (i, c) in state.iter().enumerate() {
			if i >= area.width as usize {
				break;
			}

			let x = area.x + i as u16;
			let cell = buf.get_mut(x, y);
			cell.symbol = String::from(*c);
		}
	}
}

impl EditState {
	/// Position of the editing cursor if the view is rendered in area.
	pub fn cursor(&self, area: Rect) -> XY<u16> {
		XY {
			x: area.x + self.cursor as u16,
			y: area.y,
		}
	}
}

impl Dialog for &mut EditState {
	type Output = Option<String>;

	fn handle_input(self, key: Input) -> ControlFlow<Self::Output> {
		use ControlFlow::*;

		use KeyCode::*;
		match key {
			Input(Esc, ..) => return Break(None),
			Input(Enter, ..) => return Break(Some(self.take())),
			Input(Backspace, ..) => self.pop_char_left(),
			Input(Delete, ..) => self.pop_char_right(),
			Input(Left, ..) => self.move_left(),
			Input(Right, ..) => self.move_right(),
			Input(Home, ..) => self.move_beginning(),
			Input(End, ..) => self.move_end(),
			Input(Char(c), ..) => self.insert_char(c),
			_ => debug!("Unhandled CellEditor input: {key:?}"),
		}

		return Continue(());
	}
}
