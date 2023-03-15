use std::{collections::HashMap, fmt::Debug};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::{
	input::Input,
	program::{Action, Direction},
};

type BindMap<A> = HashMap<Input, BindNode<A>>;

#[derive(Debug)]
pub enum BindNode<A> {
	Action(A),
	Chord { name: String, bindings: Bindings<A> },
}

impl<A> BindNode<A> {
	pub fn action(&self) -> Option<&A> {
		match self {
			BindNode::Action(a) => Some(a),
			_ => None,
		}
	}

	pub fn bindings(&self) -> Option<&Bindings<A>> {
		match self {
			BindNode::Chord { bindings, .. } => Some(bindings),
			_ => None,
		}
	}
}

#[derive(Debug)]
pub struct Bindings<A>(BindMap<A>);

impl Default for Bindings<Action> {
	fn default() -> Self {
		use Action::*;
		use KeyCode::*;
		let none = KeyModifiers::empty();

		let mut s = Self::empty();

		// movement
		s.insert(Input(Up, none), Move(Direction::Up));
		s.insert(Input(Down, none), Move(Direction::Down));
		s.insert(Input(Left, none), Move(Direction::Left));
		s.insert(Input(Right, none), Move(Direction::Right));
		s.insert(Input(Tab, none), Move(Direction::Right));
		s.insert(Input(Char('k'), none), Move(Direction::Up));
		s.insert(Input(Char('j'), none), Move(Direction::Down));
		s.insert(Input(Char('h'), none), Move(Direction::Left));
		s.insert(Input(Char('l'), none), Move(Direction::Right));

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

		let delete = s.create_chord("Delete", &[Input(Char('-'), KeyModifiers::ALT)]);
		delete.insert(Input(Char('c'), none), DeleteCol);
		delete.insert(Input(Char('r'), none), DeleteRow);

		let insert = s.create_chord("Insert", &[Input(Char('+'), KeyModifiers::ALT)]);
		insert.insert(Input(Char('c'), none), InsertCol);
		insert.insert(Input(Char('r'), none), InsertRow);

		s
	}
}

impl<A: Debug> Bindings<A> {
	pub fn empty() -> Self {
		Self(Default::default())
	}

	pub fn get_single(&self, k: impl Into<Input>) -> Option<&A> {
		self.0.get(&k.into()).and_then(|b| {
			if let BindNode::Action(a) = b {
				Some(a)
			} else {
				None
			}
		})
	}

	pub fn get<'a, 'b>(
		&'a self,
		inputs: impl IntoIterator<Item = &'b Input>,
	) -> Option<&'a BindNode<A>> {
		let mut inputs = inputs.into_iter().peekable();

		let mut node = self.0.get(inputs.next()?)?;
		loop {
			let Some(input) = inputs.next() else { break };
			match node {
				BindNode::Action(_) => return Some(node), // TODO: decide if exiting earlier here is correct
				BindNode::Chord { bindings, .. } => node = bindings.0.get(input)?,
			}
		}
		Some(node)
	}

	fn insert(&mut self, k: Input, v: A) {
		if let Some(n) = self.0.get(&k) {
			panic!("Input already bound: {k:?} => {n:?}");
		}
		self.0.insert(k, BindNode::Action(v));
	}

	fn create_chord<'a>(
		&mut self,
		name: &str,
		ks: impl IntoIterator<Item = &'a Input>,
	) -> &mut Bindings<A> {
		let mut map = self;
		let mut ks = ks.into_iter().peekable();
		assert!(ks.peek().is_some());
		loop {
			let Some(k) = ks.next() else { break };
			match map.0.entry(*k).or_insert(BindNode::Chord {
				name: if ks.peek().is_none() {
					name.to_string()
				} else {
					Default::default()
				},
				bindings: Self::empty(),
			}) {
				BindNode::Action(a) => panic!("Chord conflicts with existing: {a:?}"),
				BindNode::Chord { bindings, .. } => map = bindings,
			}
		}
		map
	}

	fn insert_chorded<'a>(&mut self, ks: impl IntoIterator<Item = &'a Input>, v: A) {
		let mut map = self;
		let mut ks = ks.into_iter().peekable();
		assert!(ks.peek().is_some());
		loop {
			let Some(k) = ks.next() else { break };
			if ks.peek().is_none() {
				if let Some(a) = map.0.get(k) {
					panic!("Chord already bound: {a:?}");
				}
				map.0.insert(*k, BindNode::Action(v));
				break;
			} else {
				match map.0.entry(*k).or_insert(BindNode::Chord {
					name: Default::default(),
					bindings: Self::empty(),
				}) {
					BindNode::Action(a) => panic!("Chord conflicts with existing: {a:?}"),
					BindNode::Chord { bindings, .. } => map = bindings,
				}
			}
		}
	}

	pub fn singles(&self) -> impl Iterator<Item = (&Input, &A)> {
		self.0
			.iter()
			.filter_map(|(i, n)| n.action().map(|a| (i, a)))
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn matches_single() {
		let mut b = Bindings::empty();
		b.insert(Input(KeyCode::Char('a'), KeyModifiers::NONE), ());

		assert!(b
			.get_single(Input(KeyCode::Char('a'), KeyModifiers::NONE))
			.is_some());
	}

	#[test]
	fn matches_chord() {
		let mut b = Bindings::empty();
		b.insert(Input(KeyCode::Char('a'), KeyModifiers::NONE), ());
		b.insert_chorded(
			&[
				Input(KeyCode::Char('b'), KeyModifiers::NONE),
				Input(KeyCode::Char('c'), KeyModifiers::NONE),
			],
			(),
		);

		assert!(b
			.get_single(Input(KeyCode::Char('a'), KeyModifiers::NONE))
			.is_some());

		assert!(b
			.get(&[Input(KeyCode::Char('b'), KeyModifiers::NONE),])
			.is_some());

		assert!(b
			.get(&[
				Input(KeyCode::Char('b'), KeyModifiers::NONE),
				Input(KeyCode::Char('c'), KeyModifiers::NONE)
			])
			.is_some());
	}
}
