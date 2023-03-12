use std::ops::ControlFlow;

mod edit;
pub use edit::*;
mod debug;
pub use debug::*;
mod grid;
pub use grid::*;

use crate::Input;

/// Temporary interactive widget that takes control of input.
pub trait Dialog {
	type Output;

	/// called until it returns Some(Output)
	fn handle_input(self, key: Input) -> ControlFlow<Self::Output>;
}
