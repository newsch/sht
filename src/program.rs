use std::{
	fmt::Display,
	io,
	ops::ControlFlow,
	path::{Path, PathBuf},
};

use tui::{
	backend::Backend,
	layout::{self, Constraint, Layout, Margin, Rect},
	style::{Color, Modifier, Style},
	text::Text,
	widgets::{Block, Borders, Clear, Paragraph},
	Terminal,
};

use crate::{
	grid::{ChangeTracker, Grid},
	input::{Bind, Bindings, Input, InputBuffer},
	views::{DebugView, Dialog, EditState, EditView, GridState, GridView},
	XY,
};

#[derive(Debug)]
enum Status {
	Read(PathBuf, io::Result<()>),
	Write(PathBuf, io::Result<()>),
	UndoLimit,
	RedoLimit,
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
			Status::UndoLimit => write!(f, "Nothing left to undo")?,
			Status::RedoLimit => write!(f, "Nothing left to redo")?,
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
pub struct Program {
	state: State,
	grid: Grid,
	change_tracker: ChangeTracker,
	filename: PathBuf,
	/// Store chorded keys
	input_buf: InputBuffer,
	selection: XY<usize>,
	bindings: Bindings<Action>,
	pub should_redraw: bool,
	/// Result of latest action to display to user
	status_msg: Option<Status>,
}

impl Program {
	pub fn from_path(filename: impl AsRef<Path>) -> io::Result<Self> {
		let filename = filename.as_ref().to_path_buf();

		let mut s = Self {
			filename,
			..Default::default()
		};
		s.read()?;
		// first read shouldn't be undone
		s.change_tracker = Default::default();

		Ok(s)
	}

	fn assert_selection_valid(&self) {
		let sel = self.selection;
		let size = self.grid.size();
		assert!(
			sel.x < size.x && sel.y < size.y,
			"Selection {sel:?} out of bounds {size:?}"
		);
	}

	fn handle_move(&mut self, m: Direction) {
		self.assert_selection_valid();
		use Direction::*;
		let XY { x, y } = self.selection;
		let size = self.grid.size();
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

	pub fn handle_input(&mut self, i: Input) -> io::Result<Option<ExternalAction>> {
		let action = match &mut self.state {
			State::Normal => self.handle_input_normal(i)?,
			State::EditCell(state) => {
				if let ControlFlow::Break(o) = state.handle_input(i) {
					if let Some(new_contents) = o {
						self.grid
							.edit(self.selection, new_contents)
							.track(&mut self.change_tracker);
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
		self.input_buf.push(i);
		let &action = match self.bindings.get_multiple(&self.input_buf) {
			None => {
				debug!("Unhandled input: {i}");
				self.input_buf.clear();
				return Ok(None);
			}
			Some(Bind::Partial(_)) => {
				return Ok(None);
			}
			Some(Bind::Action(a)) => a,
		};
		debug!("{} -> {action:?}", self.input_buf);
		self.input_buf.clear();

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
			Clear => {
				self.grid
					.edit(self.selection, String::new())
					.track(&mut self.change_tracker);
			}
			DeleteRow => {
				self.grid
					.delete_row(self.selection.y)
					.track(&mut self.change_tracker);
			}
			DeleteCol => {
				self.grid
					.delete_col(self.selection.x)
					.track(&mut self.change_tracker);
			}
			Undo => {
				if let None = self.change_tracker.undo(&mut self.grid) {
					self.set_status(Status::UndoLimit)
				}
			}
			Redo => {
				if let None = self.change_tracker.redo(&mut self.grid) {
					self.set_status(Status::RedoLimit)
				}
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
		let new = Grid::from_csv(rdr)?;
		self.grid.replace(new).track(&mut self.change_tracker);
		Ok(())
	}

	pub fn draw(&mut self, t: &mut Terminal<impl Backend>) -> io::Result<()> {
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
				let chord_msg = (!self.input_buf.is_empty())
					.then(|| format!("Chord: <{}> ", self.input_buf))
					.unwrap_or_default();

				let state_msg = format!(
					" {}{},{} {}x{}",
					chord_msg,
					self.selection.x + 1,
					self.selection.y + 1,
					self.grid.size().x,
					self.grid.size().y
				);

				let [status, state]: [Rect; 2] = Layout::default()
					.direction(layout::Direction::Horizontal)
					.constraints([
						Constraint::Min(0),
						Constraint::Length(state_msg.len() as u16),
					])
					.split(info)
					.try_into()
					.unwrap();
				if let Some(s) = &self.status_msg {
					f.render_widget(Paragraph::new(s).style(style), status);
				} else {
					f.render_widget(Paragraph::new("").style(style), status);
				}
				f.render_widget(Paragraph::new(state_msg).style(style), state);
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

pub enum ExternalAction {
	Quit,
}

#[derive(Debug, Copy, Clone)]
pub enum Action {
	/// Move the cursor
	Move(Direction),
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
	Undo,
	Redo,
	/// Write state to original file
	Write,
	/// Reload the original file, dropping any unsaved changes
	Read,
	/// Quit the program
	Quit,
	ToggleDebug,
}

#[derive(Debug, Copy, Clone)]
pub enum Direction {
	Up,
	Right,
	Down,
	Left,
}
