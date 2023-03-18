use std::fmt::{Debug, Display};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Input(pub KeyCode, pub KeyModifiers);

impl Ord for Input {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.0
			.partial_cmp(&other.0)
			.expect("Derived KeyCode partial_ord is actually ord 🤞")
			.then_with(|| {
				self.1
					.partial_cmp(&other.1)
					.expect("Derived KeyModifier partial_ord is actually ord 🤞")
			})
	}
}

impl From<KeyEvent> for Input {
	fn from(
		KeyEvent {
			code, modifiers, ..
		}: KeyEvent,
	) -> Self {
		Self(code, modifiers)
	}
}

impl From<KeyCode> for Input {
	fn from(code: KeyCode) -> Self {
		Self(code, KeyModifiers::NONE)
	}
}

impl From<char> for Input {
	fn from(c: char) -> Self {
		// TODO: should this be modifier shift for shift?
		Self(KeyCode::Char(c), KeyModifiers::NONE)
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

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InputBuffer(Vec<Input>);

impl InputBuffer {
	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	pub fn push(&mut self, i: Input) {
		self.0.push(i);
	}

	pub fn pop(&mut self) -> Option<Input> {
		self.0.pop()
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

impl<'a> IntoIterator for InputBuffer {
	type Item = Input;

	type IntoIter = std::vec::IntoIter<Input>;

	fn into_iter(self) -> Self::IntoIter {
		self.0.into_iter()
	}
}

impl Extend<Input> for InputBuffer {
	fn extend<T: IntoIterator<Item = Input>>(&mut self, iter: T) {
		self.0.extend(iter);
	}
}

impl FromIterator<Input> for InputBuffer {
	fn from_iter<T: IntoIterator<Item = Input>>(iter: T) -> Self {
		let mut b = Self::default();
		b.extend(iter);
		b
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
