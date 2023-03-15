//! A "simple" and straightforward terminal spreadsheet editor, in the spirit of nano and htop.
// TODO: adding/removing columns and rows
// TODO: handle different formats ala xsv
// TODO: snap edit view to cell location
// TODO: unify bindings
// TODO: online help system
// TODO: interrupt handling
// TODO: view state in debug view
// TODO: serialize and dump/reload program state
// TODO: arbitrarily nested chords
// TODO: better binding data structure, tree or similar
// TODO: draw infinite grid,
// TODO: draw frozen column/row numbers
// TODO: freeze header
use std::{error::Error, io, panic, path::PathBuf};

use crossterm::{
	cursor,
	event::{self, Event},
	execute,
	terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

#[macro_use]
extern crate log;
use structopt::StructOpt;
use tui::{
	backend::{Backend, CrosstermBackend},
	Terminal,
};

use crate::program::ExternalAction;

mod grid;
mod input;
mod logger;
mod program;
mod views;

use grid::Grid;
use program::Program;

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

fn main() -> Result<(), Box<dyn Error>> {
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
		let event = event::read()?;
		trace!("New event: {event:?}");
		let k = match event {
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
