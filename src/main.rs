//! A "simple" and straightforward terminal spreadsheet editor, in the spirit of nano and htop.
// TODO: handle different formats ala xsv
// TODO: snap edit view to cell location
// TODO: unify bindings
// TODO: online help system
// TODO: interrupt handling
// TODO: view state in debug view
// TODO: serialize and dump/reload program state
// TODO: draw infinite grid,
// TODO: draw frozen column/row numbers
// TODO: freeze header
// TODO: copy/paste
// TODO: extend binding to include mode switching, counts, type-to-edit cell
use std::{
	env,
	error::Error,
	fs::File,
	io, panic,
	path::PathBuf,
	sync::Mutex,
	time::{self, Instant},
};

use crossterm::{
	cursor::{self, SetCursorStyle},
	event::{self, Event},
	execute,
	terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

#[macro_use]
extern crate log;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use tui::{
	backend::{Backend, CrosstermBackend},
	Terminal,
};

use crate::program::ExternalAction;

mod bindings;
mod grid;
mod input;
mod logger;
mod program;
mod views;

use grid::Grid;
use program::Program;

mod styles {
	use tui::style::{Color, Modifier, Style};

	pub fn selected() -> Style {
		Style::default().add_modifier(Modifier::REVERSED)
	}

	pub fn grid() -> Style {
		Style::default().add_modifier(Modifier::UNDERLINED)
	}

	pub fn error() -> Style {
		Style::default().add_modifier(Modifier::BOLD).fg(Color::Red)
	}

	pub fn keybind() -> Style {
		Style::default().add_modifier(Modifier::BOLD)
	}
}

#[derive(Debug, StructOpt)]
struct Opt {
	#[structopt(parse(from_os_str))]
	file: PathBuf,
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct XY<T> {
	x: T,
	y: T,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rect {
	pub x: u16,
	pub y: u16,
	pub width: u16,
	pub height: u16,
}

impl Into<tui::layout::Rect> for Rect {
	fn into(self) -> tui::layout::Rect {
		let Self {
			x,
			y,
			width,
			height,
		} = self;
		tui::layout::Rect {
			x,
			y,
			width,
			height,
		}
	}
}

impl From<tui::layout::Rect> for Rect {
	fn from(value: tui::layout::Rect) -> Self {
		let tui::layout::Rect {
			x,
			y,
			width,
			height,
		} = value;
		Self {
			x,
			y,
			width,
			height,
		}
	}
}

fn setup_terminal() -> io::Result<Terminal<impl Backend>> {
	enable_raw_mode()?;
	let mut stdout = io::stdout();
	execute!(
		stdout,
		EnterAlternateScreen,
		SetCursorStyle::BlinkingBlock,
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
		SetCursorStyle::DefaultUserShape,
		// DisableMouseCapture,
		cursor::Show
	)?;
	Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
	logger::init();
	info!("Starting");

	let program = if let Ok(state_path) = env::var("FROM_STATE") {
		let f = File::open(state_path)?;
		serde_json::from_reader(f)?
	} else {
		let opt = Opt::from_args();
		Program::from_path(opt.file)?
	};

	let program = Mutex::new(program);

	match panic::catch_unwind(|| {
		let mut program = program.lock().unwrap();
		run(&mut *program)
	}) {
		Ok(r) => r?,
		Err(panic) => {
			let program = program.lock().map_or_else(|e| e.into_inner(), |l| l);
			match write_state_to_temp(&program) {
				Ok(path) => eprintln!("Captured program state at {path:?}"),
				Err(e) => eprintln!("Error writing captured state: {e}"),
			}
			panic::resume_unwind(panic);
		}
	}

	info!("Stopping");
	teardown_terminal()?;

	Ok(())
}

fn run(program: &mut Program) -> Result<(), Box<dyn Error>> {
	// reset terminal on panic
	let default_panic = panic::take_hook();
	panic::set_hook(Box::new(move |info| {
		if let Err(e) = teardown_terminal() {
			eprintln!("Error resetting terminal: {}", e);
		}
		println!();
		default_panic(info);
	}));

	let terminal = &mut setup_terminal()?;

	program.draw(terminal)?;

	loop {
		let event = event::read()?;
		trace!("New event: {event:?}");
		let k = match event {
			Event::Key(k) => k,
			Event::Resize(..) => {
				program.draw(terminal)?;
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
			program.draw(terminal)?;
		}
	}
	Ok(())
}

fn write_state_to_temp(p: &Program) -> io::Result<PathBuf> {
	let now = time::SystemTime::now()
		.duration_since(time::UNIX_EPOCH)
		.unwrap()
		.as_millis();
	let mut path = env::temp_dir();
	path.push(format!("sht_state_{now}.json"));
	let f = File::create(&path)?;
	serde_json::to_writer(f, p)?;
	Ok(path)
}
