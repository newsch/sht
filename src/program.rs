use std::{
	cmp::min,
	fmt::Display,
	io,
	ops::ControlFlow,
	path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tui::{
	backend::Backend,
	layout::{self, Constraint, Layout, Margin, Rect},
	style::{Modifier, Style},
	text::{Span, Spans, Text},
	widgets::{Block, Borders, Clear, Paragraph},
	Terminal,
};

use crate::{
	bindings::{BindNode, Bindings},
	grid::{ChangeTracker, Grid},
	input::{Input, InputBuffer},
	styles,
	views::{
		DebugView, Dialog, EditState, EditView, GridState, GridView, PaletteState, PaletteView,
	},
	Rect as MyRect, XY,
};

mod action;
pub use action::*;

#[derive(Debug, Serialize, Deserialize)]
enum Status {
	Read(
		PathBuf,
		#[serde(skip, default = "default_io_result")] io::Result<()>,
	),
	Write(
		PathBuf,
		#[serde(skip, default = "default_io_result")] io::Result<()>,
	),
	UndoLimit,
	RedoLimit,
	DumpState(#[serde(skip, default = "default_io_result")] io::Result<PathBuf>),
}

fn default_io_result<T: Default>() -> io::Result<T> {
	Ok(Default::default())
}

impl Status {
	fn err(&self) -> Option<&io::Error> {
		Some(match self {
			Status::Read(.., Err(e)) => e,
			Status::Write(.., Err(e)) => e,
			Status::DumpState(Err(e)) => e,
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
			Status::DumpState(Ok(p)) => write!(f, "Dumped state to {p:?}")?,
			Status::DumpState(Err(e)) => write!(f, "Error dumping state: {e}")?,
		}
		Ok(())
	}
}

impl<'s, 't> Into<Text<'t>> for &'s Status {
	fn into(self) -> Text<'t> {
		let base = Style::default().add_modifier(Modifier::ITALIC);
		Text::styled(
			self.to_string(),
			if self.is_err() {
				base.patch(styles::error())
			} else {
				base
			},
		)
	}
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Program {
	view: ViewState,
	grid: Grid,
	grid_state: GridState,
	change_tracker: ChangeTracker,
	filename: PathBuf,
	/// Store chorded keys
	input_buf: InputBuffer,
	selection: XY<usize>,
	/// Stored for movements based on screen size
	last_visible_grid_cells: XY<usize>,
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

	fn handle_move(&mut self, m: Direction) {
		use Direction::*;
		let XY { x, y } = self.selection;
		let s = match m {
			Up if y > 0 => XY { x, y: y - 1 },
			Down => XY { x, y: y + 1 },
			Left if x > 0 => XY { x: x - 1, y },
			Right => XY { x: x + 1, y },
			_ => return,
		};
		self.selection = s;
	}

	fn handle_jump(&mut self, m: Direction) {
		use Direction::*;
		let XY { x, y } = self.selection;
		let XY {
			x: scroll_x,
			y: scroll_y,
		} = self.grid_state.scroll();
		let XY {
			x: width,
			y: height,
			..
		} = self.grid_state.visible_cells();
		match m {
			Up => {
				self.selection.y = y.saturating_sub(height);
				self.grid_state.scroll_mut().y = scroll_y.saturating_sub(height);
			}
			Down => {
				self.selection.y = y + height;
				self.grid_state.scroll_mut().y += height;
			}
			Left => {
				// TODO: these don't take into account the width of the next columns.
				// Better method would be using the width iterator in GridView and filling the next screen...
				self.selection.x = x.saturating_sub(width);
				self.grid_state.scroll_mut().x = scroll_x.saturating_sub(width);
			}
			Right => {
				self.selection.x = x + width;
				self.grid_state.scroll_mut().x += width;
			}
		}
	}

	pub fn handle_input(&mut self, i: Input) -> io::Result<Option<ExternalAction>> {
		let action = match &mut self.view {
			ViewState::Normal => self.handle_input_normal(i)?,
			ViewState::EditCell(state) => {
				if let ControlFlow::Break(o) = state.handle_input(i) {
					if let Some(new_contents) = o {
						self.grid
							.edit(self.selection, new_contents)
							.track(&mut self.change_tracker);
					}
					self.view = ViewState::Normal;
				}
				None
			}
			ViewState::Palette(state) => {
				if let ControlFlow::Break(o) = state.handle_input(i) {
					self.view = ViewState::Normal;
					if let Some(action) = o {
						self.should_redraw = true;
						return self.handle_action(action);
					}
				}
				None
			}
			ViewState::Debug => {
				self.view = ViewState::Normal;
				None
			}
		};
		// TODO: fix this
		self.should_redraw = true;
		Ok(action)
	}

	fn handle_input_normal(&mut self, i: Input) -> io::Result<Option<ExternalAction>> {
		self.input_buf.push(i);
		let &action = match self.bindings.get(&self.input_buf) {
			None => {
				debug!("Unhandled input: {i}");
				self.input_buf.clear();
				return Ok(None);
			}
			Some(BindNode::Chord { .. }) => {
				return Ok(None);
			}
			Some(BindNode::Action(a)) => a,
		};
		debug!("{} -> {action:?}", self.input_buf);
		self.input_buf.clear();

		self.handle_action(action)
	}

	fn handle_action(&mut self, action: Action) -> io::Result<Option<ExternalAction>> {
		use Action::*;
		match action {
			Quit => return Ok(Some(ExternalAction::Quit)),
			Write => self.set_status(Status::Write(self.filename.to_owned(), self.write())),
			Read => {
				let result = self.read();
				self.set_status(Status::Read(self.filename.to_owned(), result));
			}
			Move(d) => self.handle_move(d),
			Jump(d) => self.handle_jump(d),
			Home => {
				self.selection = XY { x: 0, y: 0 };
			}
			End => {
				let XY { x, y } = self.grid.size();
				let x = x.saturating_sub(1);
				let y = y.saturating_sub(1);
				self.selection = XY { x, y };
			}
			HomeRow => {
				self.selection.x = 0;
			}
			EndRow => {
				self.selection.x = self.grid.size().x.saturating_sub(1);
			}
			HomeCol => {
				self.selection.y = 0;
			}
			EndCol => {
				self.selection.y = self.grid.size().y.saturating_sub(1);
			}
			GoTo => todo!(), // dialog
			Edit => {
				self.view = ViewState::EditCell(EditState::from_str(
					self.grid
						.get(self.selection)
						.expect("TODO: edit cells outside of grid"),
				));
				self.clear_status();
			}
			Replace => {
				self.view = ViewState::EditCell(EditState::from_str(""));
				self.clear_status();
			}
			Clear => self
				.grid
				.edit(self.selection, String::new())
				.track(&mut self.change_tracker),
			InsertRow => self
				.grid
				.insert_row(self.selection.y, Vec::new())
				.track(&mut self.change_tracker),
			InsertCol => self
				.grid
				.insert_col(self.selection.x, Vec::new())
				.track(&mut self.change_tracker),
			DeleteRow => self
				.grid
				.delete_row(self.selection.y)
				.track(&mut self.change_tracker),
			DeleteCol => self
				.grid
				.delete_col(self.selection.x)
				.track(&mut self.change_tracker),
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
				self.view = match self.view {
					ViewState::Debug => ViewState::Normal,
					_ => ViewState::Debug,
				};
			}
			TogglePalette => {
				self.view = match self.view {
					ViewState::Palette(_) => ViewState::Normal,
					_ => ViewState::Palette(PaletteState::new(&self.bindings)),
				};
			}
			DumpState => self.set_status(Status::DumpState(crate::write_state_to_temp(self))),
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
				let status_style = Style::default()
					.add_modifier(Modifier::REVERSED)
					.add_modifier(Modifier::BOLD);
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

				let [mode, status, state]: [Rect; 3] = Layout::default()
					.direction(layout::Direction::Horizontal)
					// TODO: just draw all of background and use margins/spacers
					// .horizontal_margin(1)
					.constraints([
						Constraint::Length(6),
						Constraint::Min(0),
						Constraint::Length(state_msg.len() as u16 + 1),
					])
					.split(info)
					.try_into()
					.unwrap();

				let mode_msg = match self.view {
					Normal => " VIEW ",
					EditCell(_) => " EDIT ",
					Debug => " DBUG ",
					Palette(_) => " CMDP ",
				};
				assert!(mode_msg.len() == mode.width as usize);
				f.render_widget(Paragraph::new(mode_msg).style(status_style), mode);

				if let Some(s) = &self.status_msg {
					f.render_widget(Paragraph::new(s).style(status_style), status);
				} else {
					f.render_widget(
						Paragraph::new(self.filename.to_string_lossy()).style(status_style),
						status,
					);
				}
				f.render_widget(Paragraph::new(state_msg).style(status_style), state);
			}

			let size = main;

			// sheet
			// TODO: save to keep scrolling behavior
			self.grid_state.select(Some(self.selection));
			f.render_stateful_widget(GridView::new(&self.grid), size, &mut self.grid_state);

			use ViewState::*;
			match &mut self.view {
				Normal => {
					// chord options
					if !self.input_buf.is_empty() {
						if let Some(b) = self
							.bindings
							.get(&self.input_buf)
							.and_then(|n| n.bindings())
						{
							let text: Vec<_> = b
								.singles()
								.map(|(input, a)| {
									Spans::from(vec![
										Span::styled(input.to_string(), styles::keybind()),
										Span::raw(" "),
										Span::raw(format!("{a:?}")),
									])
								})
								.collect();
							let width = min(
								size.width,
								text.iter().map(|s| s.width()).max().unwrap_or_default() as u16 + 2,
							);
							let height = min(size.height, text.len() as u16 + 2);
							let bounds = Rect {
								x: size.right() - width,
								y: size.bottom() - height,
								width,
								height,
							};
							f.render_widget(Clear, bounds);
							f.render_widget(
								Paragraph::new(text).block(
									Block::default()
										.title(Span::styled(
											format!(" {} ", self.input_buf),
											styles::keybind(),
										))
										.borders(Borders::ALL),
								),
								bounds,
							);
						}
					}
				}
				EditCell(editor) => {
					// draw edit popup
					let size = self.grid_state.selected_area().unwrap();
					f.render_widget(Clear, size);
					f.render_stateful_widget(
						EditView::default().style(styles::grid()),
						size,
						editor,
					);
					cursor_pos = Some(editor.cursor(size));
				}
				Palette(state) => {
					let margins = Margin {
						horizontal: size.width.saturating_sub(64) / 2,
						vertical: 0,
					};
					let mut size = size.inner(&margins);
					size.height = min(size.height, 32);
					f.render_stateful_widget(PaletteView::default(), size, state);
					cursor_pos = Some(state.cursor(size));
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
enum ViewState {
	/// Moving around the sheet
	#[default]
	Normal,
	/// Currently editing the selected cell
	EditCell(EditState),
	Debug,
	Palette(PaletteState),
}
