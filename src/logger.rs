use std::{
	collections::VecDeque,
	sync::Mutex,
	time::{Duration, Instant},
};

use log::{Level, Log};
use once_cell::sync::Lazy;

pub struct BufferLogger;

pub struct Record {
	pub time: Duration,
	pub level: Level,
	pub target: String,
	pub msg: String,
}

impl From<&log::Record<'_>> for Record {
	fn from(value: &log::Record<'_>) -> Self {
		let time = Instant::now().duration_since(*START);
		let level = value.level();
		let target = value.target().to_string();
		let msg = value.args().to_string();

		Self {
			time,
			level,
			target,
			msg,
		}
	}
}

pub static BUFFER: Lazy<Mutex<VecDeque<Record>>> =
	Lazy::new(|| Mutex::new(VecDeque::with_capacity(100)));

static START: Lazy<Instant> = Lazy::new(|| Instant::now());

static LOGGER: BufferLogger = BufferLogger;

pub fn init() {
	Lazy::force(&START);
	Lazy::force(&BUFFER);
	log::set_logger(&LOGGER).unwrap();
	log::set_max_level(log::LevelFilter::Trace);
}

impl Log for BufferLogger {
	fn enabled(&self, _metadata: &log::Metadata) -> bool {
		true
	}

	fn log(&self, record: &log::Record) {
		let mut buffer = BUFFER.lock().unwrap();

		if buffer.len() == buffer.capacity() {
			buffer.pop_back();
		}

		buffer.push_front(record.into());
	}

	fn flush(&self) {}
}
