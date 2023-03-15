use std::{
	collections::HashMap,
	fmt::{Debug, Display},
	io::Write,
};

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

#[derive(Debug, Hash)]
pub enum Bind<'a, A> {
	Partial(&'a [(Input, A)]),
	Action(&'a A),
}

#[derive(Debug)]
pub struct Bindings<A> {
	singles: HashMap<Input, A>,
	chords: HashMap<Input, Vec<(Input, A)>>,
}

impl Default for Bindings<Action> {
	fn default() -> Self {
		use Action::*;
		use KeyCode::*;
		let none = KeyModifiers::empty();

		let mut s = Self::empty();

		s.insert(Input(Up, none), Move(Direction::Up));
		s.insert(Input(Down, none), Move(Direction::Down));
		s.insert(Input(Left, none), Move(Direction::Left));
		s.insert(Input(Right, none), Move(Direction::Right));
		s.insert(Input(Char('c'), KeyModifiers::CONTROL), Quit);
		s.insert(Input(Char('s'), KeyModifiers::CONTROL), Write);
		s.insert(Input(Char('r'), KeyModifiers::CONTROL), Read);
		s.insert(Input(Char('z'), KeyModifiers::CONTROL), Undo);
		s.insert(Input(Char('y'), KeyModifiers::CONTROL), Redo);
		s.insert(Input(Backspace, none), Clear);
		s.insert(Input(Delete, none), Clear);
		s.insert(Input(F(2), none), Edit);
		s.insert(Input(Enter, none), Replace);
		s.insert(Input(F(12), none), ToggleDebug);

		s.insert_chorded(
			Input(Char('-'), KeyModifiers::ALT),
			Input(Char('c'), none),
			DeleteCol,
		);
		s.insert_chorded(
			Input(Char('-'), KeyModifiers::ALT),
			Input(Char('r'), none),
			DeleteRow,
		);

		s
	}
}

impl<A> Bindings<A> {
	pub fn empty() -> Self {
		Self {
			singles: Default::default(),
			chords: Default::default(),
		}
	}

	pub fn get(&self, k: impl Into<Input>) -> Option<&A> {
		self.singles.get(&k.into())
	}

	pub fn get_multiple<'a, 'b>(
		&'a self,
		inputs: impl IntoIterator<Item = &'b Input>,
	) -> Option<Bind<'a, A>> {
		let mut iter = inputs.into_iter();
		let first = iter.next()?;

		// check for single
		let single = self.singles.get(first).map(Bind::Action);
		if single.is_some() {
			return single;
		}

		// check for chord
		let chord = self.chords.get(&first)?;

		let Some(second) = iter.next() else {
			return Some(Bind::Partial(chord.as_slice()));
		};

		chord
			.iter()
			.find(|(b, _)| b == second)
			.map(|(_, a)| Bind::Action(a))
	}

	fn insert(&mut self, k: Input, v: A) {
		if self.singles.contains_key(&k) {
			panic!("Input already bound to single: {k:?}");
		}
		if self.chords.contains_key(&k) {
			panic!("Input already bound to chord: {k:?}");
		}
		self.singles.insert(k, v);
	}

	fn insert_chorded(&mut self, k1: Input, k2: Input, v: A) {
		if self.singles.contains_key(&k1) {
			panic!("Input already bound to single: {k1:?}");
		}
		let chord = self.chords.entry(k1).or_insert(Vec::new());
		for (b, _a) in chord.iter() {
			if b == &k2 {
				panic!("Chord already bound for {k1:?}, {k2:?}");
			}
		}

		chord.push((k2, v));
	}
}
