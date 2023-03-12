use tui::{
	layout::Rect,
	widgets::{List, ListItem, Widget},
};

// TODO: handle scrolling, include state
pub struct DebugView;

impl Widget for DebugView {
	fn render(self, area: Rect, buf: &mut tui::buffer::Buffer) {
		let lock = crate::logger::BUFFER.lock().unwrap();
		let items: Vec<_> = lock
			.iter()
			.take(area.height as usize)
			.map(|r| {
				ListItem::new(format!(
					"[{:5}] {: >6.2}s {}: {}",
					r.level,
					r.time.as_secs_f64(),
					r.target,
					r.msg
				))
			})
			.collect();
		let list = List::new(items);
		Widget::render(list, area, buf);
	}
}
