use std::{io, iter, mem};

use crate::XY;

use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Grid {
	cells: Vec<Vec<String>>,
	/// Dimensions of cells
	size: XY<usize>,
}

impl Grid {
	pub fn from_csv<R: io::Read>(mut rdr: csv::Reader<R>) -> io::Result<Self> {
		let records: Vec<_> = rdr.records().collect::<Result<_, _>>()?;

		let cells: Vec<Vec<_>> = records
			.into_iter()
			.map(|r| r.iter().map(|s| s.to_string()).collect())
			.collect();

		let height = cells.len();
		let width = if height == 0 { 0 } else { cells[0].len() };
		let size = XY {
			x: width,
			y: height,
		};

		Ok(Self { cells, size })
	}

	pub fn to_csv<W: io::Write>(&self, wtr: &mut csv::Writer<W>) -> io::Result<()> {
		for row in &self.cells {
			wtr.write_record(row)?;
		}
		Ok(())
	}

	pub fn cells(&self) -> &Vec<Vec<String>> {
		&self.cells
	}

	pub fn size(&self) -> XY<usize> {
		self.size
	}

	pub fn get(&self, pos: XY<usize>) -> Option<&String> {
		self.cells.get(pos.y).and_then(|r| r.get(pos.x))
	}

	fn get_mut(&mut self, pos: XY<usize>) -> Option<&mut String> {
		self.cells.get_mut(pos.y).and_then(|r| r.get_mut(pos.x))
	}

	pub fn is_in(&self, pos: XY<usize>) -> bool {
		pos.x < self.size.x && pos.y < self.size.y
	}
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ChangeTracker {
	undos: Vec<Change>,
	redos: Vec<Change>,
}

impl ChangeTracker {
	/// Record a new change, dropping any possible redos
	pub fn push(&mut self, change: Change) {
		drop(self.redos.drain(..));
		self.undos.push(change);
	}

	pub fn undo(&mut self, g: &mut Grid) -> Option<()> {
		let change = self.undos.pop()?;
		let redo = g.undo(change);
		self.redos.push(redo);
		Some(())
	}

	pub fn redo(&mut self, g: &mut Grid) -> Option<()> {
		let change = self.redos.pop()?;
		let undo = g.undo(change);
		self.undos.push(undo);
		Some(())
	}
}

/// Record of an edit to a `Grid` that contains enough information to
/// reconstruct the previous version with the current.
#[must_use = "Changes must be recorded to correctly track history"]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Change {
	Replace { pos: XY<usize>, old: String },
	ReplaceGrid { old: Grid },
	DeleteCol { col: usize, old: Vec<String> },
	InsertCol { col: usize },
	DeleteRow { row: usize, old: Vec<String> },
	InsertRow { row: usize },
}

impl Change {
	pub fn track(self, tracker: &mut ChangeTracker) {
		tracker.push(self);
	}
}

impl Grid {
	fn undo(&mut self, change: Change) -> Change {
		trace!("Undoing: {change:?}");
		use Change::*;
		match change {
			Replace { pos, old } => self.edit(pos, old),
			ReplaceGrid { old } => self.replace(old),
			DeleteCol { col, old } => self.insert_col(col, old),
			InsertCol { col } => self.delete_col(col),
			DeleteRow { row, old } => self.insert_row(row, old),
			InsertRow { row } => self.delete_row(row),
		}
	}

	pub fn replace(&mut self, other: Grid) -> Change {
		let old = mem::replace(self, other);
		Change::ReplaceGrid { old }
	}

	pub fn edit(&mut self, pos: XY<usize>, contents: String) -> Change {
		let old = mem::replace(self.get_mut(pos).unwrap(), contents);
		Change::Replace { pos, old }
	}

	pub fn insert_row(&mut self, row: usize, mut contents: Vec<String>) -> Change {
		assert!(row <= self.size.y);
		assert!(contents.len() <= self.size.x);
		if contents.len() < self.size.x {
			contents.extend(iter::repeat(String::new()).take(self.size.x - contents.len()))
		}
		self.cells.insert(row, contents);
		self.size.y += 1;
		Change::InsertRow { row }
	}

	pub fn delete_row(&mut self, row: usize) -> Change {
		assert!(row < self.size.y);
		let old = self.cells.remove(row);
		self.size.y -= 1;
		Change::DeleteRow { row, old }
	}

	pub fn insert_col(&mut self, col: usize, mut contents: Vec<String>) -> Change {
		assert!(col <= self.size.x);
		assert!(contents.len() <= self.size.y);
		if contents.len() < self.size.y {
			contents.extend(iter::repeat(String::new()).take(self.size.y - contents.len()))
		}
		for (row, text) in self.cells.iter_mut().zip(contents) {
			row.insert(col, text);
		}
		self.size.x += 1;
		Change::InsertCol { col }
	}

	pub fn delete_col(&mut self, col: usize) -> Change {
		assert!(col < self.size.x);
		let old = self.cells.iter_mut().map(|row| row.remove(col)).collect();
		self.size.x -= 1;
		Change::DeleteCol { col, old }
	}
}
