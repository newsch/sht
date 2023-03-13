use std::{
	collections::VecDeque,
	env, mem,
	sync::Mutex,
	time::{Duration, Instant},
};

use env_logger::filter::{self, Filter};
use log::{Level, LevelFilter, Log};
use once_cell::sync::OnceCell;

pub struct BufferLogger {
	buffer: Mutex<VecDeque<Record>>,
	read_buffer: Mutex<VecDeque<Record>>,
	filter: Filter,
	start: Instant,
	other: Option<env_logger::Logger>,
}

pub struct Record {
	pub time: Duration,
	pub level: Level,
	pub target: String,
	pub msg: String,
}

impl Record {
	fn new(value: &log::Record<'_>, start: Instant) -> Self {
		let time = Instant::now().duration_since(start);
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

static LOGGER: OnceCell<BufferLogger> = OnceCell::new();

/// Returns a shared buffer of logs from oldest to newest.
///
/// Logging while the buffer is locked will not cause a deadlock.
pub fn buffer() -> Option<&'static Mutex<VecDeque<Record>>> {
	let l = LOGGER.get()?;
	l.swap();
	Some(&l.read_buffer)
}

pub fn init() {
	const LOG_ENV: &str = "RUST_LOG";

	let mut filter = filter::Builder::new();
	match env::var(LOG_ENV) {
		Ok(v) if !v.trim().is_empty() => {
			filter.parse(&v);
		}
		Err(_) | Ok(_) => {
			filter.filter_level(LevelFilter::Info);
		}
	}
	let filter = filter.build();

	let max_level = filter.filter();

	let mut logger = BufferLogger::new(filter);

	if atty::isnt(atty::Stream::Stderr) {
		let other = env_logger::Builder::new().build();
		logger.with_other(other);
	}

	log::set_logger(LOGGER.get_or_init(|| logger)).unwrap();
	log::set_max_level(max_level);
	info!("Log level: {max_level}; set with {LOG_ENV:?} env var: <https://docs.rs/env_logger/#example>");
	debug!("Parsed log filters: {:?}", LOGGER.get().unwrap().filter);
}

impl BufferLogger {
	fn new(filter: Filter) -> Self {
		let buf_size = 100;
		let start = Instant::now();
		let buffer = Mutex::new(VecDeque::with_capacity(buf_size));
		let read_buffer = Mutex::new(VecDeque::with_capacity(buf_size));
		Self {
			buffer,
			read_buffer,
			start,
			filter,
			other: None,
		}
	}

	fn with_other(&mut self, other: env_logger::Logger) -> &mut Self {
		self.other = Some(other);
		self
	}

	/// Move written logs to read buffer
	fn swap(&self) {
		let mut write = self.buffer.lock().unwrap();
		let mut read = self.read_buffer.lock().unwrap();
		let num_read_free = read.capacity() - read.len();
		let num_write = write.len();
		let space_to_make = num_write.saturating_sub(num_read_free);
		drop(read.drain(..space_to_make));
		read.append(&mut write);
	}
}

impl Log for BufferLogger {
	fn enabled(&self, metadata: &log::Metadata) -> bool {
		self.filter.enabled(metadata)
	}

	fn log(&self, record: &log::Record) {
		if !self.enabled(record.metadata()) {
			return;
		}

		if let Some(other) = self.other.as_ref() {
			other.log(record);
		}

		let mut buffer = self.buffer.lock().unwrap();

		if buffer.len() == buffer.capacity() {
			buffer.pop_front();
		}

		buffer.push_back(Record::new(record, self.start));
	}

	fn flush(&self) {
		if let Some(other) = self.other.as_ref() {
			other.flush();
		}
	}
}
