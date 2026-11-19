use std::path::Path;
use tracing_subscriber::EnvFilter;

/// Initializes structured tracing for Launchify.
///
/// Uses `RUST_LOG` if set, otherwise defaults to `info`.
/// Safe to call multiple times (subsequent calls are no-ops).
pub fn init_logging(_log_dir: Option<&Path>) -> Result<(), String> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .try_init();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_logging_is_idempotent() {
        assert!(init_logging(None).is_ok());
        assert!(init_logging(None).is_ok());
    }
}
