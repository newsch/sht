use std::{
	io,
	ops::{Index, IndexMut},
};

use crate::XY;

#[derive(Default, Debug)]
pub struct Grid {
	cells: Vec<Vec<String>>,
	/// Dimensions of cells
	size: XY<usize>,
}

impl Index<XY<usize>> for Grid {
	type Output = String;

	fn index(&self, index: XY<usize>) -> &Self::Output {
		&self.cells[index.y][index.x]
	}
}

impl IndexMut<XY<usize>> for Grid {
	fn index_mut(&mut self, index: XY<usize>) -> &mut Self::Output {
		&mut self.cells[index.y][index.x]
	}
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
}
