use std::{cmp::min, ops::ControlFlow, time::Duration};

use crossterm::event::KeyCode;
use log::Level;
use tui::{
	layout::Rect,
	style::{Color, Modifier, Style},
	text::{Span, Spans},
	widgets::{List, ListItem, ListState, Paragraph, StatefulWidget, Widget},
};

use crate::Input;

use super::Dialog;

// TODO: handle scrolling, include state
pub struct DebugView;

#[derive(Default, Debug)]
pub struct DebugState {
	inner: ListState,
	/// key to find last selected log as buffer changes
	key: Option<Duration>,
	/// movement to eval on next render
	change: Option<Move>,
}

#[derive(Debug, Clone, Copy)]
enum Move {
	Up,
	Down,
	PgUp,
	PgDown,
	Home,
}

impl DebugState {
	pub fn new() -> Self {
		let mut inner = ListState::default();
		inner.select(Some(0));
		Self {
			inner,
			..Default::default()
		}
	}
}

impl StatefulWidget for DebugView {
	type State = DebugState;

	fn render(self, area: Rect, buf: &mut tui::buffer::Buffer, state: &mut Self::State) {
		let level_style = Style::default().add_modifier(Modifier::BOLD);
		let warn_style = level_style.fg(Color::Red);

		trace!("{:?}", state);

		let Some(lock) = crate::logger::buffer().map(|b| b.lock().unwrap()) else {
			let alert = Paragraph::new("Logger not initialized!").style(warn_style);
			return Widget::render(alert, area, buf);
		};

		// swap order
		state.inner.select(Some(
			lock.len() - 1 - state.inner.selected().unwrap_or_default(),
		));

		let height = area.height as usize;

		// track last selected log
		if let Some(_i) = state.inner.selected() {
			if let Some(key) = state.key {
				let i = match lock.binary_search_by_key(&key, |r| r.time) {
					Err(_) => {
						state.key = None;
						lock.len() - 1
					}
					Ok(i) => i,
				};
				state.inner.select(Some(i));
			}
		}

		// make queued movement
		if let Some(change) = state.change {
			let i = state.inner.selected().unwrap_or_default();
			let i = match change {
				Move::Up => i.saturating_sub(1),
				Move::Down => i + 1,
				Move::PgUp => i.saturating_sub(height),
				Move::PgDown => i + height,
				Move::Home => {
					// reset tracking
					state.key = None;
					0
				}
			};
			let i = min(i, lock.len() - 1);
			if i != 0 {
				state.inner.select(Some(i));
				state.key = Some(lock[i].time);
			} else {
				state.key = None;
				state.inner = Default::default();
			}
			state.change = None;
		}

		// swap order
		state.inner.select(Some(
			lock.len() - 1 - state.inner.selected().unwrap_or_default(),
		));

		let items: Vec<_> = lock
			.iter()
			.rev()
			.take(area.height as usize)
			.map(|r| {
				ListItem::new(Spans::from(vec![
					Span::raw(format!("{: >6.2}s [", r.time.as_secs_f64())),
					Span::styled(
						format!("{:5}", r.level),
						if r.level <= Level::Warn {
							warn_style
						} else {
							level_style
						},
					),
					Span::raw("] "),
					Span::styled(
						format!("{}: ", r.target),
						Style::default().add_modifier(Modifier::DIM),
					),
					Span::raw(&r.msg),
				]))
			})
			.collect();

		let list =
			List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED));
		// StatefulWidget::render(list, area, buf, &mut state.inner);
		Widget::render(list, area, buf);
	}
}

impl Dialog for &mut DebugState {
	type Output = ();

	fn handle_input(self, key: Input) -> ControlFlow<Self::Output> {
		use KeyCode::*;

		self.change = Some(match key {
			Input(Up, ..) => Move::Up,
			Input(Down, ..) => Move::Down,
			Input(PageUp, ..) => Move::PgUp,
			Input(PageDown, ..) => Move::PgDown,
			Input(Home, ..) => Move::Home,
			Input(F(12) | Esc | Char('q'), ..) => return ControlFlow::Break(()),
			_ => return ControlFlow::Continue(()),
		});
		ControlFlow::Continue(())
	}
}
