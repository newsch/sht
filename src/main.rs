//! A "simple" and straightforward terminal spreadsheet editor, in the spirit of nano and htop.
use std::{
	cmp::max,
	collections::HashMap,
	convert::TryInto,
	error::Error,
	io, mem,
	ops::{ControlFlow, Index, IndexMut},
	panic,
	path::{Path, PathBuf},
};

use crossterm::{
	cursor,
	event::{
		self, Event, KeyCode, KeyEvent, KeyModifiers,
	},
	execute,
	terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

#[macro_use]
extern crate log;
use structopt::StructOpt;
use tui::{
	backend::{Backend, CrosstermBackend},
	layout::{Constraint, Margin, Rect},
	style::{Modifier, Style},
	widgets::{
		Block, Borders, Cell, Clear, List, ListItem, Row, StatefulWidget, Table, Widget,
	},
	Terminal,
};

mod logger;

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

struct Program {
	state: State,
	grid: Grid,
	filename: PathBuf,
	selection: XY<usize>,
	bindings: Bindings,
	should_redraw: bool,
}

impl Program {
	fn from_path(filename: impl AsRef<Path>) -> io::Result<Self> {
		let filename = filename.as_ref().to_path_buf();
		let rdr = csv::ReaderBuilder::new()
			.has_headers(false)
			.from_path(&filename)?;

		let grid = Grid::from_csv(rdr)?;

		let selection = XY { x: 0, y: 0 };

		Ok(Self {
			grid,
			filename,
			state: State::Normal,
			bindings: Bindings::default(),
			should_redraw: true,
			selection,
		})
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
			Write => {
				let mut wtr = csv::Writer::from_path(&self.filename)?;
				self.grid.to_csv(&mut wtr)?;
			}
			Move(d) => self.handle_move(d),
			Edit => {
				self.state = State::EditCell(CellEditor::from_str(&self.grid[self.selection]));
			}
			Replace => {
				self.state = State::EditCell(CellEditor::from_str(""));
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

	fn draw(&mut self, t: &mut Terminal<impl Backend>) -> io::Result<()> {
		let mut cursor_pos = None;
		t.draw(|f| {
			let size = f.size();
			let block = Block::default()
				.title(self.filename.to_str().unwrap_or_default())
				.borders(Borders::ALL);
			let inner = block.inner(size);
			f.render_widget(block, size);
			let mut state = GridState {
				selection: self.selection,
			};
			f.render_stateful_widget(&self.grid, inner, &mut state);

			use State::*;
			match &self.state {
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
					f.render_widget(editor, inner);
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

#[derive(Debug, Clone)]
enum State {
	/// Moving around the sheet
	Normal,
	/// Currently editing the selected cell
	EditCell(CellEditor),
	Debug,
}

// TODO: use grapheme clusters instead...
#[derive(Debug, Clone)]
struct CellEditor {
	buffer: Vec<char>,
	/// [0, buffer.len()]
	cursor: usize,
}

impl CellEditor {
	fn from_str(s: &str) -> Self {
		let buffer: Vec<_> = s.chars().collect();
		Self {
			cursor: buffer.len(),
			buffer,
		}
	}

	/// Iterator of current chars
	fn iter(&self) -> impl Iterator<Item = &char> {
		self.buffer.iter()
	}

	/// Remove the character right of the cursor.
	fn pop_char_right(&mut self) {
		if self.cursor >= self.buffer.len() {
			return;
		}
		self.buffer.remove(self.cursor);
	}

	/// Remove the character left of the cursor.
	fn pop_char_left(&mut self) {
		if self.cursor <= 0 {
			return;
		}
		self.buffer.remove(self.cursor - 1);
		self.cursor -= 1;
	}

	/// Insert a character at the current position.
	fn insert_char(&mut self, c: char) {
		self.buffer.insert(self.cursor, c);
		self.cursor += 1;
	}

	fn move_left(&mut self) {
		if self.cursor <= 0 {
			return;
		}
		self.cursor -= 1;
	}

	fn move_right(&mut self) {
		if self.cursor >= self.buffer.len() {
			return;
		}
		self.cursor += 1;
	}

	fn move_beginning(&mut self) {
		self.cursor = 0;
	}

	fn move_end(&mut self) {
		self.cursor = self.buffer.len();
	}

	/// Remove the contents as a string
	fn take(&mut self) -> String {
		mem::take(&mut self.buffer).into_iter().collect()
	}
}

impl Widget for &CellEditor {
	fn render(self, area: Rect, buf: &mut tui::buffer::Buffer) {
		// TODO: handle overflow w/ ellipses
		let y = area.y;
		for (i, c) in self.iter().enumerate() {
			if i >= area.width as usize {
				break;
			}

			let x = area.x + i as u16;
			let cell = buf.get_mut(x, y);
			cell.symbol = String::from(*c);
		}
	}
}

impl CellEditor {
	fn cursor(&self, area: Rect) -> XY<u16> {
		XY {
			x: area.x + self.cursor as u16,
			y: area.y,
		}
	}
}

impl Dialog for &mut CellEditor {
	type Output = Option<String>;

	fn handle_input(self, key: Input) -> ControlFlow<Self::Output> {
		use ControlFlow::*;

		use KeyCode::*;
		match key {
			Input(Esc, ..) => return Break(None),
			Input(Enter, ..) => return Break(Some(self.take())),
			Input(Backspace, ..) => self.pop_char_left(),
			Input(Delete, ..) => self.pop_char_right(),
			Input(Left, ..) => self.move_left(),
			Input(Right, ..) => self.move_right(),
			Input(Home, ..) => self.move_beginning(),
			Input(End, ..) => self.move_end(),
			Input(Char(c), ..) => self.insert_char(c),
			_ => debug!("Unhandled CellEditor input: {key:?}"),
		}

		return Continue(());
	}
}

/// Temporary interactive widget that takes control of input.
trait Dialog {
	type Output;

	/// called until it returns Some(Output)
	fn handle_input(self, key: Input) -> ControlFlow<Self::Output>;
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
	/// Quit the program
	Quit,
	ToggleDebug,
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
		m.insert(Input(Char('w'), none), Write);
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

struct Grid {
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

struct GridState {
	selection: XY<usize>,
}

impl StatefulWidget for &Grid {
	type State = GridState;

	fn render(
		self,
		area: tui::layout::Rect,
		buf: &mut tui::buffer::Buffer,
		state: &mut Self::State,
	) {
		let table = Table::new(self.cells.iter().enumerate().map(|(y, row)| {
			let row = row.clone();
			if y == state.selection.y {
				let mut row: Vec<_> = row.into_iter().map(Cell::from).collect();
				row[state.selection.x] = row[state.selection.x]
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

		Widget::render(table, area, buf);
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

struct DebugView;

impl Widget for DebugView {
	fn render(self, area: Rect, buf: &mut tui::buffer::Buffer) {
		let lock = logger::BUFFER.lock().unwrap();
		let items: Vec<_> = lock
			.iter()
			.take(area.height as usize)
			.map(|r| {
				ListItem::new(format!(
					"[{:5}] {: >6.2}s {}: {}",
					r.level,
					r.time.as_secs_f64(),
					r.target,
					r.msg
				))
			})
			.collect();
		let list = List::new(items);
		Widget::render(list, area, buf);
	}
}

fn main() -> Result<(), Box<dyn Error>> {
	// TODO: in-memory logger
	logger::init();

	let opt = Opt::from_args();

	info!("Hello, world!");
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

	teardown_terminal()?;

	Ok(())
}
