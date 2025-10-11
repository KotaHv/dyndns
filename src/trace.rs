use chrono::{Local, SecondsFormat};
use tracing_subscriber::{
    filter::Targets,
    fmt::{self, time},
    prelude::*,
};
use yansi::Paint;

use crate::CONFIG;

pub fn init() {
    let is_color = CONFIG.log.style.is_color();
    if !is_color {
        yansi::disable();
    }
    let format = fmt::layer().with_timer(LocalTime).with_ansi(is_color);
    let level = CONFIG.log.level.as_str();
    let filter: Targets = match level.parse() {
        Ok(f) => f,
        Err(e) => {
            let err = format!("string {} did not parse successfully: {}", level, e);
            panic!("{}", err.red().bold());
        }
    };

    tracing_subscriber::registry()
        .with(format)
        .with(filter)
        .init();
}

struct LocalTime;

impl time::FormatTime for LocalTime {
    fn format_time(&self, w: &mut fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(
            w,
            "{}",
            Local::now().to_rfc3339_opts(SecondsFormat::Millis, false)
        )
    }
}
