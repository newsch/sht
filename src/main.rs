//! A "simple" and straightforward terminal spreadsheet editor, in the spirit of nano and htop.
// TODO: undo/redo
// TODO: adding/removing columns and rows
// TODO: handle different formats ala xsv
// TODO: snap edit view to cell location
// TODO: unify bindings
// TODO: online help system
// TODO: interrupt handling
use std::{
	collections::HashMap,
	error::Error,
	fmt::Display,
	io,
	ops::{ControlFlow, Index, IndexMut},
	panic,
	path::{Path, PathBuf},
};

use crossterm::{
	cursor,
	event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
	execute,
	terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

#[macro_use]
extern crate log;
use structopt::StructOpt;
use tui::{
	backend::{Backend, CrosstermBackend},
	layout::{self, Constraint, Layout, Margin, Rect},
	style::{Color, Modifier, Style},
	text::Text,
	widgets::{Block, Borders, Clear, Paragraph},
	Terminal,
};
use views::{Dialog, EditState, EditView};

use crate::views::{DebugView, GridState, GridView};

mod logger;
mod views;

#[derive(Debug, StructOpt)]
struct Opt {
	#[structopt(parse(from_os_str))]
	file: PathBuf,
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct XY<T> {
	x: T,
	y: T,
}

#[derive(Debug)]
enum Status {
	Read(PathBuf, io::Result<()>),
	Write(PathBuf, io::Result<()>),
}

impl Status {
	fn err(&self) -> Option<&io::Error> {
		Some(match self {
			Status::Read(.., Err(e)) => e,
			Status::Write(.., Err(e)) => e,
			_ => return None,
		})
	}

	fn is_err(&self) -> bool {
		self.err().is_some()
	}
}

impl Display for Status {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Status::Read(p, Ok(())) => write!(f, "Read from {p:?}")?,
			Status::Read(p, Err(e)) => write!(f, "Error reading from {p:?}: {e}")?,
			Status::Write(p, Ok(())) => write!(f, "Wrote to {p:?}")?,
			Status::Write(p, Err(e)) => write!(f, "Error writing to {p:?}: {e}")?,
		}
		Ok(())
	}
}

impl<'s, 't> Into<Text<'t>> for &'s Status {
	fn into(self) -> Text<'t> {
		let style = Style::default();

		Text::styled(
			self.to_string(),
			if self.is_err() {
				style.fg(Color::Red)
			} else {
				style
			},
		)
	}
}

#[derive(Default, Debug)]
struct Program {
	state: State,
	grid: Grid,
	filename: PathBuf,
	selection: XY<usize>,
	bindings: Bindings,
	should_redraw: bool,
	/// Result of latest action to display to user
	status_msg: Option<Status>,
}

impl Program {
	fn from_path(filename: impl AsRef<Path>) -> io::Result<Self> {
		let filename = filename.as_ref().to_path_buf();

		let mut s = Self {
			filename,
			..Default::default()
		};
		s.read()?;

		Ok(s)
	}

	fn assert_selection_valid(&self) {
		let sel = self.selection;
		let size = self.grid.size;
		assert!(
			sel.x < size.x && sel.y < size.y,
			"Selection {sel:?} out of bounds {size:?}"
		);
	}

	fn handle_move(&mut self, m: Direction) {
		self.assert_selection_valid();
		use Direction::*;
		let XY { x, y } = self.selection;
		let size = self.grid.size;
		let s = match m {
			Up if y > 0 => XY { x, y: y - 1 },
			Down if y < size.y - 1 => XY { x, y: y + 1 },
			Left if x > 0 => XY { x: x - 1, y },
			Right if x < size.x - 1 => XY { x: x + 1, y },
			_ => return,
		};
		self.selection = s;
		self.assert_selection_valid();
	}

	fn handle_input(&mut self, i: Input) -> io::Result<Option<ExternalAction>> {
		let action = match &mut self.state {
			State::Normal => self.handle_input_normal(i)?,
			State::EditCell(state) => {
				if let ControlFlow::Break(o) = state.handle_input(i) {
					if let Some(new_contents) = o {
						self.grid[self.selection] = new_contents;
					}
					self.state = State::Normal;
				}
				None
			}
			State::Debug => {
				self.state = State::Normal;
				None
			}
		};
		// TODO: fix this
		self.should_redraw = true;
		Ok(action)
	}

	fn handle_input_normal(&mut self, i: Input) -> io::Result<Option<ExternalAction>> {
		let Some(action) = self.bindings.get(i) else {
			debug!("Unhandled input: {i:?}");
			return Ok(None);
		};

		use Action::*;
		match action {
			Quit => return Ok(Some(ExternalAction::Quit)),
			Write => self.set_status(Status::Write(self.filename.to_owned(), self.write())),
			Read => self.status_msg = Some(Status::Read(self.filename.to_owned(), self.read())),
			Move(d) => self.handle_move(d),
			Edit => {
				self.state = State::EditCell(EditState::from_str(&self.grid[self.selection]));
				self.clear_status();
			}
			Replace => {
				self.state = State::EditCell(EditState::from_str(""));
				self.clear_status();
			}
			ToggleDebug => {
				self.state = match self.state {
					State::Debug => State::Normal,
					_ => State::Debug,
				};
			}
		}

		Ok(None)
	}

	fn set_status(&mut self, status: Status) {
		if status.is_err() {
			error!("{}", status);
		} else {
			info!("{status}");
		}
		self.status_msg = Some(status);
	}

	fn clear_status(&mut self) {
		self.status_msg = None;
	}

	fn write(&self) -> io::Result<()> {
		let mut wtr = csv::Writer::from_path(&self.filename)?;
		self.grid.to_csv(&mut wtr)?;
		Ok(())
	}

	fn read(&mut self) -> io::Result<()> {
		let rdr = csv::ReaderBuilder::new()
			.has_headers(false)
			.from_path(&self.filename)?;
		self.grid = Grid::from_csv(rdr)?;
		Ok(())
	}

	fn draw(&mut self, t: &mut Terminal<impl Backend>) -> io::Result<()> {
		let mut cursor_pos = None;
		trace!("Beginning draw");
		t.draw(|f| {
			let [main, info]: [Rect; 2] = Layout::default()
				.direction(layout::Direction::Vertical)
				.constraints(vec![Constraint::Min(1), Constraint::Length(1)])
				.split(f.size())
				.try_into()
				.unwrap();

			// status bar
			{
				let style = Style::default().add_modifier(Modifier::REVERSED);
				let pos_msg = format!("{},{}", self.selection.x + 1, self.selection.y + 1);

				let [status, pos]: [Rect; 2] = Layout::default()
					.direction(layout::Direction::Horizontal)
					.constraints([Constraint::Min(0), Constraint::Length(pos_msg.len() as u16)])
					.split(info)
					.try_into()
					.unwrap();
				if let Some(s) = &self.status_msg {
					f.render_widget(Paragraph::new(s).style(style), status);
				} else {
					f.render_widget(Paragraph::new("").style(style), status);
				}
				f.render_widget(Paragraph::new(pos_msg).style(style), pos);
			}

			let size = main;

			// sheet
			let block = Block::default()
				.title(self.filename.to_str().unwrap_or_default())
				.borders(Borders::ALL);
			let inner = block.inner(size);
			f.render_widget(block, size);
			let mut state = GridState::default();
			state.select(self.selection);
			f.render_stateful_widget(GridView::new(&self.grid), inner, &mut state);

			use State::*;
			match &mut self.state {
				Normal => {}
				EditCell(editor) => {
					// draw edit popup
					let margins = Margin {
						horizontal: 8,
						vertical: 10,
					};
					let size = size.inner(&margins);
					let border = Block::default()
						.title(format!("{:?}", self.selection))
						.borders(Borders::ALL);
					let inner = border.inner(size);
					f.render_widget(Clear, size);
					f.render_widget(border, size);
					f.render_stateful_widget(EditView::default(), inner, editor);
					cursor_pos = Some(editor.cursor(inner));
				}
				Debug => {
					let border = Block::default().title("Logs").borders(Borders::ALL);
					let inner = border.inner(size);
					f.render_widget(Clear, size);
					f.render_widget(border, size);
					f.render_widget(DebugView, inner);
				}
			}
		})?;

		if let Some(XY { x, y }) = cursor_pos {
			t.set_cursor(x, y)?;
			t.show_cursor()?;
		} else {
			t.hide_cursor()?;
		}

		self.should_redraw = false;
		Ok(())
	}
}

#[derive(Debug, Clone, Default)]
enum State {
	/// Moving around the sheet
	#[default]
	Normal,
	/// Currently editing the selected cell
	EditCell(EditState),
	Debug,
}

enum ExternalAction {
	Quit,
}

#[derive(Debug, Copy, Clone)]
enum Action {
	/// Move the cursor
	Move(Direction),
	/// Edit the current cell
	Edit,
	/// Replace the current cell
	Replace,
	/// Write state to original file
	Write,
	/// Reload the original file, dropping any unsaved changes
	Read,
	/// Quit the program
	Quit,
	ToggleDebug,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Hash)]
pub struct Input(KeyCode, KeyModifiers);

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
		m.insert(Input(Char('c'), KeyModifiers::CONTROL), Quit);
		m.insert(Input(Char('s'), KeyModifiers::CONTROL), Write);
		m.insert(Input(Char('r'), KeyModifiers::CONTROL), Read);
		m.insert(Input(F(2), none), Edit);
		m.insert(Input(Enter, none), Replace);
		m.insert(Input(F(12), none), ToggleDebug);

		Self(m)
	}
}

impl Bindings {
	fn get(&self, k: impl Into<Input>) -> Option<Action> {
		let k = k.into();
		self.0.get(&k).map(|v| *v)
	}
}

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
	fn from_csv<R: io::Read>(mut rdr: csv::Reader<R>) -> io::Result<Self> {
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

	fn to_csv<W: io::Write>(&self, wtr: &mut csv::Writer<W>) -> io::Result<()> {
		for row in &self.cells {
			wtr.write_record(row)?;
		}
		Ok(())
	}
}

fn setup_terminal() -> io::Result<Terminal<impl Backend>> {
	enable_raw_mode()?;
	let mut stdout = io::stdout();
	execute!(
		stdout,
		EnterAlternateScreen,
		// EnableMouseCapture
	)?;
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
		// DisableMouseCapture,
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
	logger::init();
	info!("Starting");

	let opt = Opt::from_args();

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

	let mut program = Program::from_path(opt.file)?;

	program.draw(&mut terminal)?;

	loop {
		let k = match event::read()? {
			Event::Key(k) => k,
			Event::Resize(..) => {
				program.draw(&mut terminal)?;
				continue;
			}
			e => {
				debug!("Unhandled event: {e:?}");
				continue;
			}
		};

		if let Some(action) = program.handle_input(k.into())? {
			match action {
				ExternalAction::Quit => break,
			}
		}

		if program.should_redraw {
			program.draw(&mut terminal)?;
		}
	}

	info!("Stopping");
	teardown_terminal()?;

	Ok(())
}
