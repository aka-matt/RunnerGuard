//! Static validation of an [`AppConfig`]. Used by `runnerguard config
//! validate` and right after loading.

use crate::app_config::AppConfig;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationIssue {
    #[error("network.timeout_seconds must be > 0")]
    BadTimeout,
    #[error("network.connect_timeout_seconds must be > 0")]
    BadConnectTimeout,
    #[error("network.retry_count must be <= 10")]
    BadRetryCount,
    #[error("limits.max_file_bytes must be > 0")]
    BadMaxFileBytes,
    #[error("limits.max_xml_depth must be > 0")]
    BadMaxXmlDepth,
    #[error("limits.max_project_files must be > 0")]
    BadMaxProjectFiles,
    #[error("ai.api_key is unset; required because ai.enabled = true")]
    AiKeyMissing,
    #[error("ai.base_url is required when ai.enabled = true")]
    AiBaseUrlMissing,
    #[error("ai.model is required when ai.enabled = true")]
    AiModelMissing,
    #[error("network.proxy_url is not a valid URL: {0}")]
    BadProxyUrl(String),
    #[error("ai.temperature must be between 0.0 and 2.0")]
    BadTemperature,
}

/// Collect every problem with `config` so users can fix them all at once.
pub fn validate(config: &AppConfig) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if config.network.timeout_seconds == 0 {
        issues.push(ValidationIssue::BadTimeout);
    }
    if config.network.connect_timeout_seconds == 0 {
        issues.push(ValidationIssue::BadConnectTimeout);
    }
    if config.network.retry_count > 10 {
        issues.push(ValidationIssue::BadRetryCount);
    }
    if config.limits.max_file_bytes == 0 {
        issues.push(ValidationIssue::BadMaxFileBytes);
    }
    if config.limits.max_xml_depth == 0 {
        issues.push(ValidationIssue::BadMaxXmlDepth);
    }
    if config.limits.max_project_files == 0 {
        issues.push(ValidationIssue::BadMaxProjectFiles);
    }

    if config.ai.enabled {
        if config.ai.api_key.is_unset() {
            issues.push(ValidationIssue::AiKeyMissing);
        }
        if config.ai.base_url.as_deref().unwrap_or("").is_empty() {
            issues.push(ValidationIssue::AiBaseUrlMissing);
        }
        if config.ai.model.as_deref().unwrap_or("").is_empty() {
            issues.push(ValidationIssue::AiModelMissing);
        }
    }

    if !(0.0..=2.0).contains(&config.ai.temperature) {
        issues.push(ValidationIssue::BadTemperature);
    }

    if let Some(proxy) = &config.network.proxy_url {
        if url::Url::parse(proxy).is_err() {
            issues.push(ValidationIssue::BadProxyUrl(proxy.clone()));
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{ConfigSource, load_from_str};

    #[test]
    fn clean_default_passes() {
        let loaded = load_from_str("version: 1\n", ConfigSource::Stdin).unwrap();
        assert!(validate(&loaded.config).is_empty());
    }

    #[test]
    fn ai_enabled_without_key_fails() {
        let yaml = r#"
version: 1
ai:
  enabled: true
  base_url: https://api.example.com
  model: gpt-x
  api_key:
    env: ""
"#;
        let loaded = load_from_str(yaml, ConfigSource::Stdin).unwrap();
        let issues = validate(&loaded.config);
        assert!(
            issues
                .iter()
                .any(|i| matches!(i, ValidationIssue::AiKeyMissing))
        );
    }
}
