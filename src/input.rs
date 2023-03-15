use std::fmt::{Debug, Display};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Hash)]
pub struct Input(pub KeyCode, pub KeyModifiers);

impl From<KeyEvent> for Input {
	fn from(
		KeyEvent {
			code, modifiers, ..
		}: KeyEvent,
	) -> Self {
		Self(code, modifiers)
	}
}

impl Display for Input {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let Self(code, modifiers) = self;

		// TODO: handle other modifiers?
		if modifiers.contains(KeyModifiers::CONTROL) {
			write!(f, "Ctrl ")?;
		}
		if modifiers.contains(KeyModifiers::ALT) {
			write!(f, "Alt ")?;
		}
		if modifiers.contains(KeyModifiers::SHIFT) {
			write!(f, "Shift ")?;
		}

		match code {
			KeyCode::F(num) => write!(f, "F{num}")?,
			KeyCode::Char(c) => write!(f, "{}", c)?,
			_ => write!(f, "{:?}", code)?,
		}

		Ok(())
	}
}

#[derive(Debug, Default, Clone)]
pub struct InputBuffer(Vec<Input>);

impl InputBuffer {
	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	pub fn push(&mut self, i: Input) {
		self.0.push(i);
	}

	pub fn clear(&mut self) {
		self.0.drain(..);
	}
}

impl<'a> IntoIterator for &'a InputBuffer {
	type Item = &'a Input;

	type IntoIter = std::slice::Iter<'a, Input>;

	fn into_iter(self) -> Self::IntoIter {
		self.0.iter()
	}
}

impl Display for InputBuffer {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		for (i, input) in self.into_iter().enumerate() {
			if i != 0 {
				write!(f, ", ")?;
			}
			write!(f, "{}", input)?;
		}
		Ok(())
	}
}
