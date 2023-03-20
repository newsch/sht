use std::fmt::Display;

use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use strum::{EnumMessage, IntoStaticStr};

pub enum ExternalAction {
	Quit,
}

#[derive(
	Debug,
	Copy,
	Clone,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	Hash,
	Sequence,
	EnumMessage,
	IntoStaticStr,
	Serialize,
	Deserialize,
)]
pub enum Action {
	/// Move the cursor
	Move(Direction),
	Jump(Direction),
	GoTo,
	Home,
	End,
	HomeCol,
	EndCol,
	HomeRow,
	EndRow,
	/// Edit the current cell
	Edit,
	/// Replace the current cell
	Replace,
	/// Clear the current cell
	Clear,
	/// Delete column of current cursor
	DeleteCol,
	/// Delete row of current cursor
	DeleteRow,
	/// Insert column of current cursor
	InsertCol,
	/// Insert row of current cursor
	InsertRow,
	Undo,
	Redo,
	/// Write state to original file
	Write,
	/// Reload the original file, dropping any unsaved changes
	Read,
	/// Quit the program
	Quit,
	ToggleDebug,
	DumpState,
	TogglePalette,
}

impl Action {
	// TODO: make detailed for Move and other sub-variants, probably with mem::discriminant and lazy_static?
	pub fn desc(&self) -> &'static str {
		self.get_documentation().unwrap_or(self.into())
	}
}

pub struct Doc<'a>(&'a Action);

impl<'a> Display for Doc<'a> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if let Some(d) = self.0.get_documentation() {
			write!(f, "{}", d)?;
		} else {
			write!(f, "{:?}", self.0)?;
		}

		Ok(())
	}
}

#[derive(
	Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Sequence, Serialize, Deserialize,
)]
pub enum Direction {
	Down,
	Left,
	Right,
	Up,
}
