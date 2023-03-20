use std::{
	collections::{hash_map, HashMap},
	fmt::Debug,
};

use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};

use crate::{
	input::{Input, InputBuffer},
	program::{Action, Direction},
};

type BindMap<A> = HashMap<Input, BindNode<A>>;

#[derive(Debug, Serialize, Deserialize)]
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

impl<A> Serialize for Bindings<A>
where
	A: Serialize,
{
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		let list: Vec<_> = self.0.iter().collect();
		list.serialize(serializer)
	}
}

impl<'de, A> Deserialize<'de> for Bindings<A>
where
	A: Deserialize<'de>,
{
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		let list = Vec::<(Input, BindNode<A>)>::deserialize(deserializer)?;
		let mut value = HashMap::new();
		value.extend(list.into_iter());
		Ok(Bindings(value))
	}
}

impl Default for Bindings<Action> {
	fn default() -> Self {
		use Action as A;
		use Direction as D;
		use KeyCode::*;
		let none = KeyModifiers::empty();

		let mut s = Self::empty();

		// movement
		s.insert(Input(Up, none), A::Move(D::Up));
		s.insert(Input(Down, none), A::Move(D::Down));
		s.insert(Input(Left, none), A::Move(D::Left));
		s.insert(Input(Right, none), A::Move(D::Right));
		s.insert(Input(Tab, none), A::Move(D::Right));
		s.insert(Input(Char('k'), none), A::Move(D::Up));
		s.insert(Input(Char('j'), none), A::Move(D::Down));
		s.insert(Input(Char('h'), none), A::Move(D::Left));
		s.insert(Input(Char('l'), none), A::Move(D::Right));
		s.insert(Input(Home, KeyModifiers::CONTROL), A::Home);
		s.insert(Input(End, KeyModifiers::CONTROL), A::End);
		s.insert(Input(Home, none), A::HomeRow);
		s.insert(Input(End, none), A::EndRow);
		s.insert(Input(PageUp, none), A::Jump(D::Up));
		s.insert(Input(PageDown, none), A::Jump(D::Down));
		s.insert(Input(PageUp, KeyModifiers::ALT), A::Jump(D::Left));
		s.insert(Input(PageDown, KeyModifiers::ALT), A::Jump(D::Right));
		s.insert(Input(Char('g'), KeyModifiers::CONTROL), A::GoTo);

		s.insert(Input(Char('c'), KeyModifiers::CONTROL), A::Quit);
		s.insert(Input(Char('s'), KeyModifiers::CONTROL), A::Write);
		s.insert(Input(Char('r'), KeyModifiers::CONTROL), A::Read);
		s.insert(Input(Char('z'), KeyModifiers::CONTROL), A::Undo);
		s.insert(Input(Char('y'), KeyModifiers::CONTROL), A::Redo);
		s.insert(Input(Backspace, none), A::Clear);
		s.insert(Input(Delete, none), A::Clear);
		s.insert(Input(F(2), none), A::Edit);
		s.insert(Input(Enter, none), A::Replace);
		s.insert(Input(F(12), none), A::ToggleDebug);
		s.insert(Input(F(1), none), A::TogglePalette);

		let delete = s.create_chord("Delete", &[Input(Char('-'), KeyModifiers::ALT)]);
		delete.insert(Input(Char('c'), none), A::DeleteCol);
		delete.insert(Input(Char('r'), none), A::DeleteRow);

		let insert = s.create_chord("Insert", &[Input(Char('+'), KeyModifiers::ALT)]);
		insert.insert(Input(Char('c'), none), A::InsertCol);
		insert.insert(Input(Char('r'), none), A::InsertRow);

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

	pub fn insert(&mut self, k: Input, v: A) {
		if let Some(n) = self.0.get(&k) {
			panic!("Input already bound: {k:?} => {n:?}");
		}
		self.0.insert(k, BindNode::Action(v));
	}

	pub fn create_chord<'a>(
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

	pub fn iter(&self) -> impl Iterator<Item = (InputBuffer, &A)> {
		Iter::new(self)
	}

	pub fn actions(&self) -> impl Iterator<Item = &A> {
		// TODO: standalone with fewer allocations
		Iter::new(self).map(|(_i, a)| a)
	}
}

/// Iterator that walks the binding tree in a DFS, yielding each chord's actions before deeper chords
#[derive(Debug)]
struct Iter<'a, A> {
	queue: Vec<(InputBuffer, &'a Bindings<A>)>,
	current: Option<hash_map::Iter<'a, Input, BindNode<A>>>,
	buf: InputBuffer,
}

impl<'a, A> Iter<'a, A> {
	fn new(b: &'a Bindings<A>) -> Self {
		Self {
			current: Some(b.0.iter()),
			queue: Default::default(),
			buf: InputBuffer::default(),
		}
	}
}

impl<'a, A: Debug> Iterator for Iter<'a, A> {
	type Item = (InputBuffer, &'a A);

	fn next(&mut self) -> Option<Self::Item> {
		loop {
			let Some(current) = &mut self.current else {
				let Some((input, bindings)) = self.queue.pop() else {
					return None;
				};
				self.buf.extend(input);
				self.current = Some(bindings.0.iter());
				continue;
			};
			match current.next() {
				None => {
					self.current = None;
					self.buf.pop();
					continue;
				}
				Some((input, BindNode::Action(action))) => {
					let mut buf = self.buf.clone();
					buf.push(*input);
					return Some((buf, action));
				}
				Some((input, BindNode::Chord { bindings, .. })) => {
					let mut buf = self.buf.clone();
					buf.push(*input);
					self.queue.push((buf, bindings));
					continue;
				}
			}
		}
	}
}

#[cfg(test)]
mod test {
	use std::collections::{hash_map::RandomState, HashSet};

	use super::*;

	fn example() -> Bindings<usize> {
		let mut b = Bindings::empty();
		b.insert(Input(KeyCode::Char('a'), KeyModifiers::NONE), 1);
		b.insert_chorded(
			&[
				Input(KeyCode::Char('b'), KeyModifiers::NONE),
				Input(KeyCode::Char('c'), KeyModifiers::NONE),
			],
			2,
		);
		b.insert_chorded(
			&[
				Input(KeyCode::Char('b'), KeyModifiers::NONE),
				Input(KeyCode::Char('d'), KeyModifiers::NONE),
				Input(KeyCode::Char('e'), KeyModifiers::NONE),
			],
			3,
		);

		b
	}

	#[test]
	fn matches_single() {
		let b = example();
		assert_eq!(
			Some(&1),
			b.get_single(Input(KeyCode::Char('a'), KeyModifiers::NONE))
		);
	}

	#[test]
	fn matches_chord() {
		let b = example();

		assert!(matches!(
			b.get(&[Input(KeyCode::Char('b'), KeyModifiers::NONE)]),
			Some(&BindNode::Chord { .. })
		));

		assert_eq!(
			Some(&2),
			b.get(&[
				Input(KeyCode::Char('b'), KeyModifiers::NONE),
				Input(KeyCode::Char('c'), KeyModifiers::NONE),
			])
			.and_then(BindNode::action)
		);

		assert_eq!(
			Some(&3),
			b.get(&[
				Input(KeyCode::Char('b'), KeyModifiers::NONE),
				Input(KeyCode::Char('d'), KeyModifiers::NONE),
				Input(KeyCode::Char('e'), KeyModifiers::NONE),
			])
			.and_then(BindNode::action)
		);
	}

	#[test]
	fn iters_all() {
		let b = example();
		let actions: Vec<_> = b.iter().collect();

		let expected: Vec<(InputBuffer, &usize)> = vec![
			(
				[Input(KeyCode::Char('a'), KeyModifiers::NONE)]
					.into_iter()
					.collect(),
				&1,
			),
			(
				[
					Input(KeyCode::Char('b'), KeyModifiers::NONE),
					Input(KeyCode::Char('c'), KeyModifiers::NONE),
				]
				.into_iter()
				.collect(),
				&2,
			),
			(
				[
					Input(KeyCode::Char('b'), KeyModifiers::NONE),
					Input(KeyCode::Char('d'), KeyModifiers::NONE),
					Input(KeyCode::Char('e'), KeyModifiers::NONE),
				]
				.into_iter()
				.collect(),
				&3,
			),
		];

		assert_eq!(expected.len(), actions.len());
		let expected_set: HashSet<_, RandomState> = HashSet::from_iter(expected);
		let actions_set: HashSet<_, RandomState> = HashSet::from_iter(actions);
		assert_eq!(expected_set, actions_set)
	}
}
