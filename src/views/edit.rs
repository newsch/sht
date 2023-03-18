use std::{
	mem,
	ops::ControlFlow::{self, *},
};

use crossterm::event::KeyCode;
use serde::{Deserialize, Serialize};
use tui::{layout::Rect, style::Style, widgets::StatefulWidget};

use crate::{bindings::Bindings, input::Input, program::Direction, XY};

use super::Dialog;

#[derive(Default)]
pub struct EditView {
	style: Style,
}

impl EditView {
	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub enum EditAction {
	Char(char),
	Backspace,
	Delete,
	Enter,
	Move(Direction),
	Cancel,
	Submit,
	Jump(Direction),
}

impl EditAction {
	pub fn bindings() -> Bindings<Self> {
		let mut b = Bindings::empty();
		use EditAction as A;
		use KeyCode::*;

		b.insert(Esc.into(), A::Cancel);
		b.insert(Enter.into(), A::Enter);
		b.insert(Backspace.into(), A::Backspace);
		b.insert(Delete.into(), A::Delete);
		b.insert(Left.into(), A::Move(Direction::Left));
		b.insert(Right.into(), A::Move(Direction::Right));
		b.insert(Up.into(), A::Move(Direction::Up));
		b.insert(Down.into(), A::Move(Direction::Down));
		b.insert(Home.into(), A::Jump(Direction::Left));
		b.insert(End.into(), A::Jump(Direction::Right));

		b
	}
}

// TODO: use chars/grapheme clusters instead...
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct EditState {
	buffer: String,
	/// [0, buffer.len()]
	cursor: usize,
}

impl EditState {
	pub fn from_str(s: &str) -> Self {
		let buffer = s.to_string();
		Self {
			cursor: buffer.len(),
			buffer,
		}
	}

	/// Reference of the current text being edited
	pub fn contents(&self) -> &str {
		&self.buffer
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
		mem::take(&mut self.buffer)
	}
}

impl StatefulWidget for EditView {
	type State = EditState;

	fn render(self, area: Rect, buf: &mut tui::buffer::Buffer, state: &mut Self::State) {
		// TODO: handle overflow w/ ellipses
		buf.set_style(area, self.style);
		buf.set_stringn(
			area.x,
			area.y,
			state.contents(),
			area.width as usize,
			Style::default(),
		);
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
		let bindings = EditAction::bindings();
		let action = match key {
			Input(KeyCode::Char(c), ..) => EditAction::Char(c),
			_ => {
				let Some(a) = bindings.get_single(key) else {
					debug!("Unhandled CellEditor input: {key:?}");
					return Continue(());
				};
				*a
			}
		};

		self.handle_input(action)
	}
}

impl Dialog<EditAction> for &mut EditState {
	type Output = Option<String>;

	fn handle_input(self, action: EditAction) -> ControlFlow<Self::Output> {
		use ControlFlow::*;

		// TODO: multiline
		use Direction::*;
		use EditAction::*;
		match action {
			Cancel => return Break(None),
			Submit | Enter => return Break(Some(self.take())),
			Backspace => self.pop_char_left(),
			Delete => self.pop_char_right(),
			Move(Left) => self.move_left(),
			Move(Right) => self.move_right(),
			Move(Up) | Jump(Up) | Jump(Left) => self.move_beginning(),
			Move(Down) | Jump(Down) | Jump(Right) => self.move_end(),
			Char(c) => self.insert_char(c),
		}

		return Continue(());
	}
}
