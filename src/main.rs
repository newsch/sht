//! A "simple" and straightforward terminal spreadsheet editor, in the spirit of nano and htop.
use std::{
	cmp::max,
	collections::HashMap,
	convert::TryInto,
	error::Error,
	io, panic,
	path::{Path, PathBuf},
};

use crossterm::{
	cursor,
	event::{
		self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
	},
	execute,
	terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

#[macro_use]
extern crate log;
use structopt::StructOpt;
use tui::{
	backend::{Backend, CrosstermBackend},
	layout::Constraint,
	style::{Modifier, Style},
	widgets::{Block, Borders, Cell, Row, Table},
	Terminal,
};

#[derive(Debug, StructOpt)]
struct Opt {
	#[structopt(parse(from_os_str))]
	file: PathBuf,
}

#[derive(Debug, Copy, Clone)]
struct XY<T> {
	x: T,
	y: T,
}

#[derive(Debug, Copy, Clone)]
enum Action {
	Move(Direction),
	Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Hash)]
struct Input(KeyCode, KeyModifiers);

impl From<KeyEvent> for Input {
	fn from(
		KeyEvent {
			code, modifiers, ..
		}: KeyEvent,
	) -> Self {
		Self(code, modifiers)
	}
}

struct Bindings(HashMap<Input, Action>);

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
		m.insert(Input(Char('q'), none), Quit);

		Self(m)
	}
}

impl Bindings {
	fn get(&self, k: impl Into<Input>) -> Option<Action> {
		let k = k.into();
		self.0.get(&k).map(|v| *v)
	}
}

struct Grid {
	filename: PathBuf,
	cells: Vec<Vec<String>>,
	/// Dimensions of cells
	size: XY<usize>,
	selection: XY<usize>,
}

impl Grid {
	fn from_path(filename: impl AsRef<Path>) -> io::Result<Self> {
		let filename = filename.as_ref().to_path_buf();
		let mut rdr = csv::ReaderBuilder::new()
			.has_headers(false)
			.from_path(&filename)?;

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

		let selection = XY { x: 0, y: 0 };

		Ok(Self {
			filename,
			cells,
			size,
			selection,
		})
	}

	fn draw(&self, t: &mut Terminal<impl Backend>) -> io::Result<()> {
		let table = Table::new(self.cells.iter().enumerate().map(|(y, row)| {
			let row = row.clone();
			if y == self.selection.y {
				let mut row: Vec<_> = row.into_iter().map(Cell::from).collect();
				row[self.selection.x] = row[self.selection.x]
					.clone()
					.style(Style::default().add_modifier(Modifier::BOLD));
				Row::new(row)
			} else {
				Row::new(row)
			}
		}));
		// highlight selected

		// use longest width
		let constraints = self
			.cells
			.iter()
			.fold(vec![0; self.cells.len()], |mut len, row| {
				for (i, cell) in row.iter().enumerate() {
					len[i] = max(len[i], cell.len());
				}
				len
			})
			.into_iter()
			.map(|l| l.try_into().expect("assume cell width less that u16 max"))
			.map(|l| max(l, 16))
			.map(Constraint::Length)
			.collect::<Vec<_>>();

		let table = table.widths(&constraints);

		t.draw(|f| {
			let size = f.size();
			let block = Block::default()
				.title(self.filename.to_str().unwrap_or_default())
				.borders(Borders::ALL);
			let inner = block.inner(size);
			f.render_widget(block, size);
			f.render_widget(table, inner);
		})?;
		Ok(())
	}

	fn assert_selection_valid(&self) {
		let sel = self.selection;
		let size = self.size;
		assert!(
			sel.x < size.x && sel.y < size.y,
			"Selection {sel:?} out of bounds {size:?}"
		);
	}

	fn handle_move(&mut self, m: Direction) {
		self.assert_selection_valid();
		use Direction::*;
		let XY { x, y } = self.selection;
		let s = match m {
			Up if self.selection.y > 0 => XY { x, y: y - 1 },
			Down if self.selection.y < self.size.y - 1 => XY { x, y: y + 1 },
			Left if self.selection.x > 0 => XY { x: x - 1, y },
			Right if self.selection.x < self.size.x - 1 => XY { x: x + 1, y },
			_ => return,
		};
		self.selection = s;
		self.assert_selection_valid();
	}
}

fn setup_terminal() -> io::Result<Terminal<impl Backend>> {
	enable_raw_mode()?;
	let mut stdout = io::stdout();
	execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
	let backend = CrosstermBackend::new(stdout);
	Terminal::new(backend)
}

fn teardown_terminal() -> io::Result<()> {
	// restore terminal
	disable_raw_mode()?;
	let mut stdout = io::stdout();
	execute!(
		stdout,
		LeaveAlternateScreen,
		DisableMouseCapture,
		cursor::Show
	)?;
	Ok(())
}

#[derive(Debug, Copy, Clone)]
enum Direction {
	Up,
	Right,
	Down,
	Left,
}

fn main() -> Result<(), Box<dyn Error>> {
	// TODO: in-memory logger

	let opt = Opt::from_args();

	info!("Hello, world!");
	let mut grid = Grid::from_path(opt.file)?;
	let bindings = Bindings::default();

	let mut terminal = setup_terminal()?;

	// reset terminal on panic
	let default_panic = panic::take_hook();
	panic::set_hook(Box::new(move |info| {
		if let Err(e) = teardown_terminal() {
			eprintln!("Error resetting terminal: {}", e);
		}
		println!();
		default_panic(info);
	}));

	grid.draw(&mut terminal)?;

	loop {
		let k = match event::read()? {
			Event::Key(k) => k,
			Event::Resize(..) => {
				grid.draw(&mut terminal)?;
				continue;
			}
			e => {
				debug!("Unhandled event: {e:?}");
				continue;
			}
		};

		let Some(action) = bindings.get(k) else {
			debug!("Unhandled key: {k:?}");
			continue;
		};

		use Action::*;
		match action {
			Move(d) => grid.handle_move(d),
			Quit => break,
		}
		grid.draw(&mut terminal)?;
	}

	teardown_terminal()?;

	Ok(())
}
