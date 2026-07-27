//! Render an `AppConfig` as YAML with secrets redacted. The output is
//! safe to print on the terminal, log, or attach to a bug report.

use crate::app_config::AppConfig;
use serde::Serialize;
use std::fmt::Write;

/// Re-serialised config with every `SecretRef` replaced by
/// `***REDACTED***`. This is what `runnerguard config show` prints.
#[derive(Debug, Serialize)]
struct RedactedAiConfig<'a> {
    enabled: bool,
    provider: &'a str,
    base_url: Option<&'a str>,
    model: Option<&'a str>,
    api_key: &'static str,
    timeout_seconds: u64,
    max_retries: u32,
    max_input_characters: usize,
    temperature: f32,
    send_source_code: bool,
    redact_secrets: bool,
}

#[derive(Debug, Serialize)]
struct RedactedAppConfig<'a> {
    version: u32,
    scan: &'a crate::app_config::ScanConfig,
    limits: &'a crate::app_config::LimitsConfig,
    network: &'a crate::app_config::NetworkConfig,
    ai: RedactedAiConfig<'a>,
    report: &'a crate::app_config::ReportConfig,
    logging: &'a crate::app_config::LoggingConfig,
}

pub fn render_redacted_yaml(config: &AppConfig) -> Result<String, serde_yaml_ng::Error> {
    let redacted = RedactedAppConfig {
        version: config.version,
        scan: &config.scan,
        limits: &config.limits,
        network: &config.network,
        ai: RedactedAiConfig {
            enabled: config.ai.enabled,
            provider: &config.ai.provider,
            base_url: config.ai.base_url.as_deref(),
            model: config.ai.model.as_deref(),
            api_key: "***REDACTED***",
            timeout_seconds: config.ai.timeout_seconds,
            max_retries: config.ai.max_retries,
            max_input_characters: config.ai.max_input_characters,
            temperature: config.ai.temperature,
            send_source_code: config.ai.send_source_code,
            redact_secrets: config.ai.redact_secrets,
        },
        report: &config.report,
        logging: &config.logging,
    };

    let mut buf = String::new();
    let yaml = serde_yaml_ng::to_string(&redacted)?;
    let _ = writeln!(&mut buf, "{yaml}");
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{ConfigSource, load_from_str};

    #[test]
    fn renders_with_redacted_secret() {
        let loaded = load_from_str(
            "version: 1\nai:\n  api_key:\n    value: super-secret\n",
            ConfigSource::Stdin,
        )
        .unwrap();
        let yaml = render_redacted_yaml(&loaded.config).unwrap();
        assert!(yaml.contains("***REDACTED***"));
        assert!(!yaml.contains("super-secret"));
    }
}
