use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::config::{LogConfig, LogFormat};

pub fn init(cfg: &LogConfig) {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&cfg.level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let registry = tracing_subscriber::registry().with(filter);

    match cfg.format {
        LogFormat::Json => {
            let _ = registry
                .with(fmt::layer().json().with_target(true))
                .try_init();
        },
        LogFormat::Pretty => {
            let _ = registry
                .with(fmt::layer().compact().with_target(true))
                .try_init();
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_is_idempotent() {
        let cfg = LogConfig {
            level: "info".into(),
            format: LogFormat::Pretty,
        };
        init(&cfg);
        init(&cfg);
    }
}
