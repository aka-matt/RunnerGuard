//! The default config YAML written by `runnerguard config init`.

pub const DEFAULT_CONFIG_YAML: &str = r#"version: 1

scan:
  default_rules: null
  output_directory: "./runnerguard-report"
  include_tests: false
  continue_on_parse_error: true
  fail_on: "error"
  write_flow_json: true

limits:
  max_file_bytes: 10485760
  max_project_files: 50000
  max_xml_depth: 256
  max_http_response_bytes: 10485760
  follow_symlinks: false

network:
  offline: false
  timeout_seconds: 30
  connect_timeout_seconds: 10
  retry_count: 3
  proxy_url: null
  extra_ca_file: null
  allow_hosts: []
  credentials: {}

ai:
  enabled: false
  provider: "openai-compatible"
  base_url: null
  model: null
  api_key:
    env: "RUNNERGUARD_AI_API_KEY"
  timeout_seconds: 60
  max_retries: 2
  max_input_characters: 120000
  temperature: 0.0
  send_source_code: false
  redact_secrets: true

report:
  formats:
    - "markdown"
    - "html"
  include_absolute_paths: false
  include_flow_json_links: true
  include_ai_section: true

logging:
  level: "info"
  file: null
  json: false
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{ConfigSource, load_from_str};

    #[test]
    fn default_yaml_round_trips() {
        let loaded = load_from_str(DEFAULT_CONFIG_YAML, ConfigSource::Default).unwrap();
        assert!(!loaded.config.ai.enabled);
        assert_eq!(loaded.config.ai.provider, "openai-compatible");
        assert_eq!(
            loaded.config.scan.fail_on,
            runnerguard_model::Severity::Error
        );
    }
}
