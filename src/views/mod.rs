use std::ops::ControlFlow;

mod edit;
pub use edit::*;
mod debug;
pub use debug::*;
mod grid;
pub use grid::*;
mod table;
use table::*;
mod palette;
pub use palette::*;

use crate::input;

/// Temporary interactive widget that takes control of input.
pub trait Dialog<Input = input::Input> {
	type Output;

	/// called until it returns Some(Output)
	fn handle_input(self, input: Input) -> ControlFlow<Self::Output>;
}
