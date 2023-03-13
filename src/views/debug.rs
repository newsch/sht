


use log::Level;
use tui::{
	layout::Rect,
	style::{Color, Modifier, Style},
	text::{Span, Spans},
	widgets::{List, ListItem, Paragraph, Widget},
};





// TODO: handle scrolling, include state
pub struct DebugView;

impl Widget for DebugView {
	fn render(self, area: Rect, buf: &mut tui::buffer::Buffer) {
		let level_style = Style::default().add_modifier(Modifier::BOLD);
		let warn_style = level_style.fg(Color::Red);

		let Some(lock) = crate::logger::buffer().map(|b| b.lock().unwrap()) else {
			let alert = Paragraph::new("Logger not initialized!").style(warn_style);
			return Widget::render(alert, area, buf);
		};

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
		Widget::render(list, area, buf);
	}
}
