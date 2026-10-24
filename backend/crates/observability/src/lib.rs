//! One stderr JSON subscriber; stdout remains available for worker/CLI protocols.
use std::error::Error;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{filter::filter_fn, layer::SubscriberExt};

pub fn init() -> Result<(), Box<dyn Error + Send + Sync>> {
    let level = match std::env::var("LOG_LEVEL") {
        Ok(value) => value
            .parse::<LevelFilter>()
            .map_err(|_| "LOG_LEVEL must be off/error/warn/info/debug/trace")?,
        Err(std::env::VarError::NotPresent) => LevelFilter::INFO,
        Err(_) => return Err("LOG_LEVEL must be valid UTF-8".into()),
    };
    let subscriber = tracing_subscriber::registry()
        // Third-party debug logs may contain SQL, URLs, mail addresses or credentials.
        .with(filter_fn(|m| m.target().starts_with("rapidmaker_")))
        .with(level)
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(std::io::stderr)
                .with_current_span(true)
                .with_span_list(true),
        );
    tracing::subscriber::set_global_default(subscriber)?;
    std::panic::set_hook(Box::new(|panic| {
        if let Some(location) = panic.location() {
            tracing::error!(
                event = "panic",
                file = location.file(),
                line = location.line(),
                column = location.column()
            );
        } else {
            tracing::error!(event = "panic");
        }
        // Panic payloads can contain application data; never format them into diagnostics.
    }));
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn json_filter_and_panic_hook_protect_process_output() {
        let run = |level| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "tests::process_probe",
                    "--ignored",
                    "--nocapture",
                ])
                .env("LOG_LEVEL", level)
                .output()
                .unwrap()
        };
        let output = run("trace");
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("protocol_canary"));
        let logs = String::from_utf8(output.stderr).unwrap();
        for secret in ["driver_canary", "panic_canary"] {
            assert!(!logs.contains(secret));
        }
        let events: Vec<serde_json::Value> = logs
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert!(events.iter().any(|e| e["fields"]["event"] == "panic"));
        assert!(
            events
                .iter()
                .any(|e| e["fields"]["event"] == "probe" && e["span"]["job_id"] == "safe-id")
        );
        let quiet = run("off");
        assert!(quiet.status.success() && quiet.stderr.is_empty());
        let invalid = run("invalid_level_canary");
        assert!(!invalid.status.success());
        assert!(!String::from_utf8_lossy(&invalid.stderr).contains("invalid_level_canary"));
    }

    #[test]
    #[ignore = "subprocess fixture invoked by the logging contract test"]
    fn process_probe() {
        super::init().expect("subscriber configuration");
        let span = tracing::info_span!("job", job_id = "safe-id");
        let _entered = span.enter();
        tracing::info!(event = "probe");
        tracing::error!(target:"sqlx", "driver_canary");
        assert!(std::panic::catch_unwind(|| panic!("panic_canary")).is_err());
        println!("protocol_canary");
    }
}
