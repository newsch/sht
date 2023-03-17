// TODO: rework edit around string/graphemes
use std::{
	cmp::min,
	collections::HashSet,
	iter,
	ops::ControlFlow::{self, *},
};

use crossterm::event::{KeyCode, KeyModifiers};
use enum_iterator;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use strum::EnumMessage;
use tui::{
	buffer::Buffer,
	layout::{Alignment, Constraint, Direction::Vertical, Layout, Rect},
	text::{Span, Spans, Text},
	widgets::{
		Block, Borders, Clear, List, ListItem, ListState, Paragraph, StatefulWidget, Widget,
	},
};

use crate::{
	bindings::Bindings,
	input::{Input, InputBuffer},
	program::Action,
	styles, XY,
};

use super::{Dialog, EditState, EditView};

type Item = (Option<InputBuffer>, Action);

#[derive(Debug, Clone)]
pub struct PaletteState {
	items: Vec<Item>,
	edit: EditState,
	list: ListState,
}

impl PaletteState {
	pub fn new(bindings: &Bindings<Action>) -> Self {
		let items = Self::generate_list(bindings);
		let mut list = ListState::default();
		list.select(Some(0));
		Self {
			items,
			list,
			edit: Default::default(),
		}
	}

	pub fn cursor(&self, area: Rect) -> XY<u16> {
		// HACK
		let mut c = self.edit.cursor(area);
		c.x += 1;
		c.y += 1;
		c
	}

	fn matching(&self) -> impl Iterator<Item = &Item> {
		let query = self.edit.contents();
		let is_empty = query.trim().is_empty();
		let matcher = SkimMatcherV2::default();
		self.items
			.iter()
			.filter(move |(_i, a)| is_empty || matcher.fuzzy_match(a.desc(), query).is_some())
		// TODO: order by weight
	}

	fn generate_list(bindings: &Bindings<Action>) -> Vec<Item> {
		let mut items: Vec<_> = bindings
			.iter()
			.map(|(i, a)| (Some(i), a.to_owned()))
			.collect();
		let bound: HashSet<_> = items.iter().map(|(_i, a)| *a).collect();
		let all: HashSet<_> = enum_iterator::all::<Action>().collect();
		let unbound = all.difference(&bound);
		items.extend(unbound.into_iter().map(|a| (None, *a)));
		// TODO: maybe skip sets?
		// TODO: handle multiple bindings to the same action
		items.sort_unstable_by_key(|(_i, a)| *a);
		items.dedup_by_key(|(_i, a)| *a);
		items
	}

	fn map_selection(&mut self, f: impl FnOnce(usize) -> usize) {
		let selection = self.list.selected().unwrap_or_default();
		let updated = f(selection);
		self.list.select(Some(updated));
	}

	fn move_down(&mut self) {
		let bottom = self.matching().count() - 1;
		self.map_selection(|s| min(bottom, s.saturating_add(1)));
	}

	fn move_up(&mut self) {
		// TODO: wrap?
		self.map_selection(|s| s.saturating_sub(1));
	}

	fn jump_down(&mut self) {
		let bottom = self.matching().count() - 1;
		self.map_selection(|_s| bottom);
	}

	fn jump_up(&mut self) {
		self.map_selection(|_s| 0);
	}

	fn selected(&self) -> Option<Action> {
		self.matching()
			.map(|(_i, a)| *a)
			.nth(self.list.selected().unwrap_or_default())
	}

	fn reset_selection(&mut self) {
		self.list = ListState::default();
		self.list.select(Some(0));
	}
}

#[derive(Default, Debug)]
pub struct PaletteView {}

impl StatefulWidget for PaletteView {
	type State = PaletteState;

	fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
		let [input, results]: [Rect; 2] = Layout::default()
			.constraints([Constraint::Length(3), Constraint::Min(2)])
			.direction(Vertical)
			.split(area)
			.try_into()
			.unwrap();
		// input box
		{
			let area = input;
			Clear.render(area, buf);
			let block = Block::default()
				.title("Command Palette")
				.title_alignment(Alignment::Center)
				.borders(Borders::ALL);
			let inner = block.inner(area);
			block.render(area, buf);
			EditView::default().render(inner, buf, &mut state.edit);
		}
		// search results, highlighted
		{
			let block = Block::default().borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT);
			let mut area = results;
			let borders_height = 1;
			let borders_width = 2;
			let items: Vec<_> = state
				.matching()
				.map(|(i, a)| {
					let mut desc = a
						.get_documentation()
						.map(|d| d.to_string())
						.unwrap_or_else(|| format!("{:?}", a));
					let bind = i.to_owned().map(|i| i.to_string()).unwrap_or_default();
					let min_sep = 1;
					let desc_width =
						(area.width as usize).saturating_sub(borders_width + bind.len() + min_sep);

					if desc.len() >= desc_width {
						desc.truncate(desc_width.saturating_sub(1));
						desc.push('…');
					}

					let spacing = desc_width.saturating_sub(desc.len());

					ListItem::new(Spans::from(vec![
						Span::raw(desc),
						Span::raw(String::from_iter(iter::repeat(' ').take(spacing))),
						Span::styled(bind, styles::keybind()),
					]))
				})
				.collect();
			if items.is_empty() {
				let mut area = area;
				area.height = min(area.height, 2);
				Clear.render(area, buf);
				Paragraph::new(Text::styled("No results", styles::error()))
					.block(block)
					.alignment(Alignment::Center)
					.render(area, buf);
				return;
			}
			let displayed = items.len() as u16;
			let list = List::new(items)
				.block(block)
				.highlight_style(styles::selected());
			area.height = min(area.height, displayed + borders_height);
			Clear.render(area, buf);
			StatefulWidget::render(list, area, buf, &mut state.list)
		}
	}
}

impl Dialog for &mut PaletteState {
	type Output = Option<Action>;

	fn handle_input(self, key: Input) -> ControlFlow<Self::Output> {
		match key {
			Input(KeyCode::Up, ..) => {
				self.move_up();
				return Continue(());
			}
			Input(KeyCode::Down, ..) => {
				self.move_down();
				return Continue(());
			}
			Input(KeyCode::Home, KeyModifiers::CONTROL) => {
				self.jump_up();
				return Continue(());
			}
			Input(KeyCode::End, KeyModifiers::CONTROL) => {
				self.jump_down();
				return Continue(());
			}
			Input(KeyCode::Char(_), ..) => {
				// reset on input change
				self.reset_selection();
			}
			Input(KeyCode::Enter, ..) => {
				return Break(self.selected());
			}
			_ => {}
		}
		if let Break(_) = self.edit.handle_input(key) {
			return Break(None);
		}
		Continue(())
	}
}
