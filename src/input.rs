use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::program::{Action, Direction};

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

#[derive(Debug)]
pub struct Bindings(HashMap<Input, Action>);

impl Default for Bindings {
	fn default() -> Self {
		use Action::*;
		use KeyCode::*;
		let none = KeyModifiers::empty();

		let mut m = HashMap::new();
		m.insert(Input(Up, none), Move(Direction::Up));
		m.insert(Input(Down, none), Move(Direction::Down));
		m.insert(Input(Left, none), Move(Direction::Left));
		m.insert(Input(Right, none), Move(Direction::Right));
		m.insert(Input(Char('c'), KeyModifiers::CONTROL), Quit);
		m.insert(Input(Char('s'), KeyModifiers::CONTROL), Write);
		m.insert(Input(Char('r'), KeyModifiers::CONTROL), Read);
		m.insert(Input(Char('z'), KeyModifiers::CONTROL), Undo);
		m.insert(Input(Char('y'), KeyModifiers::CONTROL), Redo);
		m.insert(Input(Backspace, none), Clear);
		m.insert(Input(Delete, none), Clear);
		m.insert(Input(F(2), none), Edit);
		m.insert(Input(Enter, none), Replace);
		m.insert(Input(F(12), none), ToggleDebug);

		Self(m)
	}
}

impl Bindings {
	pub fn get(&self, k: impl Into<Input>) -> Option<Action> {
		let k = k.into();
		self.0.get(&k).map(|v| *v)
	}
}
