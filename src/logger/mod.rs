// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Result;

use tracing::{debug, info, warn};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_journald as journald;
use tracing_subscriber::{EnvFilter, filter::Directive, fmt, prelude::*};

pub fn init_logger() -> Result<()> {
    let level = if cfg!(debug_assertions) {
        "debug"
    } else {
        "warn"
    };

    // Console/journald filter - respects RUST_LOG or defaults to warn in release
    let console_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            EnvFilter::new(if cfg!(debug_assertions) {
                "info"
            } else {
                "warn"
            })
        })
        .add_directive(Directive::from_str("cosmic_text=error").unwrap())
        .add_directive(Directive::from_str("calloop=error").unwrap())
        .add_directive(Directive::from_str(&format!("smithay={level}")).unwrap())
        .add_directive(Directive::from_str(&format!("cosmic_comp={level}")).unwrap());

    let fmt_layer = fmt::layer().compact().with_filter(console_filter);

    // Add file logging to Documents folder
    let documents_path = dirs::document_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let log_path_display = documents_path.display().to_string();

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("cosmic-comp-keyboard")
        .filename_suffix("log")
        .max_log_files(7)
        .build(documents_path)
        .ok();

    // Create a separate filter for the file that always includes keyboard-related logs
    let file_filter = EnvFilter::new("info")
        .add_directive(Directive::from_str("cosmic_text=error").unwrap())
        .add_directive(Directive::from_str("calloop=error").unwrap())
        .add_directive(Directive::from_str("smithay::input=info").unwrap())
        .add_directive(Directive::from_str("cosmic_comp=info").unwrap());

    match (journald::layer(), file_appender) {
        (Ok(journald_layer), Some(file_appender)) => {
            let file_layer = fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .with_filter(file_filter);

            tracing_subscriber::registry()
                .with(fmt_layer)
                .with(journald_layer)
                .with(file_layer)
                .init();

            info!(
                "Keyboard logs will be written to: {}/cosmic-comp-keyboard.log",
                log_path_display
            );
        }
        (Ok(journald_layer), None) => {
            tracing_subscriber::registry()
                .with(fmt_layer)
                .with(journald_layer)
                .init();
            warn!("Failed to create file appender for keyboard logging.");
        }
        (Err(err), Some(file_appender)) => {
            let file_layer = fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .with_filter(file_filter);

            tracing_subscriber::registry()
                .with(fmt_layer)
                .with(file_layer)
                .init();

            warn!(?err, "Failed to init journald logging.");
            info!(
                "Keyboard logs will be written to: {}/cosmic-comp-keyboard.log",
                log_path_display
            );
        }
        (Err(err), None) => {
            tracing_subscriber::registry().with(fmt_layer).init();
            warn!(?err, "Failed to init journald logging.");
            warn!("Failed to create file appender for keyboard logging.");
        }
    };
    log_panics::init();

    info!("Version: {}", std::env!("CARGO_PKG_VERSION"));
    if cfg!(feature = "debug") {
        debug!(
            "Debug build ({})",
            std::option_env!("GIT_HASH").unwrap_or("Unknown")
        );
    }

    Ok(())
}
