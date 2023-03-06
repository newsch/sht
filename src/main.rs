use std::{error::Error, fs, path::PathBuf};

use cursive::{
	theme::{self, Palette, Theme},
	traits::*,
};
use cursive_table_view::{TableView, TableViewItem};
use log::*;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
	#[structopt(parse(from_os_str))]
	file: PathBuf,
}

#[derive(Debug, Clone)]
struct Row(csv::StringRecord);

impl TableViewItem<usize> for Row {
	fn to_column(&self, column: usize) -> String {
		if column == 0 {
			self.0.position().unwrap().line().to_string()
		} else {
			self.0.get(column - 1).unwrap().to_string()
		}
	}

	fn cmp(&self, other: &Self, column: usize) -> std::cmp::Ordering
	where
		Self: Sized,
	{
		if column == 0 {
			self.0
				.position()
				.unwrap()
				.line()
				.cmp(&other.0.position().unwrap().line())
		} else {
			let column = column - 1;
			self.0.get(column).cmp(&other.0.get(column))
		}
	}
}

fn main() -> Result<(), Box<dyn Error>> {
	cursive::logger::init();

	let opt = Opt::from_args();

	let input_file = fs::File::open(opt.file)?;

	info!("Hello, world!");
	let mut rdr = csv::Reader::from_reader(input_file);
	let headers = rdr.headers().unwrap();
	debug!("Headers: {:?}", headers);

	let theme = Theme {
		shadow: false,
		borders: theme::BorderStyle::Simple,
		palette: Palette::default(),
	};

	let mut siv = cursive::default();
	siv.set_theme(theme);

	siv.add_global_callback('q', |s| s.quit());
	siv.add_global_callback('~', |s| s.toggle_debug_console());

	let mut table = TableView::<Row, usize>::new().column(0, "#", |c| c);

	for (i, h) in headers.into_iter().enumerate() {
		table = table.column(i + 1, h, |c| c);
	}
	for result in rdr.records() {
		let record = result.unwrap();
		table.insert_item(Row(record));
	}

	// add_fullscreen_layer removes shadow
	// full_screen() makes table view as large as possible
	siv.add_fullscreen_layer(table.full_screen());

	siv.run();

	Ok(())
}
