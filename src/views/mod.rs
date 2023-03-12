mod edit;
use std::ops::ControlFlow;

pub use edit::*;
mod debug;
pub use debug::*;

use crate::Input;

/// Temporary interactive widget that takes control of input.
pub trait Dialog {
	type Output;

	/// called until it returns Some(Output)
	fn handle_input(self, key: Input) -> ControlFlow<Self::Output>;
}
