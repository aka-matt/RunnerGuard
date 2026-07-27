//! AppConfig: the on-disk shape of a runnerguard config file.

use runnerguard_model::{RuleFilter, SecretRef, Severity};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub limits: LimitsConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub report: ReportConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            scan: ScanConfig::default(),
            limits: LimitsConfig::default(),
            network: NetworkConfig::default(),
            ai: AiConfig::default(),
            report: ReportConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

fn default_version() -> u32 {
    1
}

pub type VersionConfig = u32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanConfig {
    #[serde(default)]
    pub default_rules: Option<Vec<PathBuf>>,
    #[serde(default = "default_output_dir")]
    pub output_directory: PathBuf,
    #[serde(default)]
    pub include_tests: bool,
    #[serde(default = "default_continue_on_parse_error")]
    pub continue_on_parse_error: bool,
    #[serde(default = "default_fail_on")]
    pub fail_on: Severity,
    #[serde(default = "default_write_flow_json")]
    pub write_flow_json: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            default_rules: None,
            output_directory: PathBuf::from("./runnerguard-report"),
            include_tests: false,
            continue_on_parse_error: true,
            fail_on: Severity::Error,
            write_flow_json: true,
        }
    }
}

fn default_output_dir() -> PathBuf {
    PathBuf::from("./runnerguard-report")
}
fn default_continue_on_parse_error() -> bool {
    true
}
fn default_fail_on() -> Severity {
    Severity::Error
}
fn default_write_flow_json() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LimitsConfig {
    #[serde(default = "default_max_file_bytes")]
    pub max_file_bytes: u64,
    #[serde(default = "default_max_project_files")]
    pub max_project_files: usize,
    #[serde(default = "default_max_xml_depth")]
    pub max_xml_depth: usize,
    #[serde(default = "default_max_http_response_bytes")]
    pub max_http_response_bytes: u64,
    #[serde(default)]
    pub follow_symlinks: bool,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_file_bytes: 10 * 1024 * 1024,
            max_project_files: 50_000,
            max_xml_depth: 256,
            max_http_response_bytes: 10 * 1024 * 1024,
            follow_symlinks: false,
        }
    }
}

fn default_max_file_bytes() -> u64 {
    10 * 1024 * 1024
}
fn default_max_project_files() -> usize {
    50_000
}
fn default_max_xml_depth() -> usize {
    256
}
fn default_max_http_response_bytes() -> u64 {
    10 * 1024 * 1024
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetworkConfig {
    #[serde(default)]
    pub offline: bool,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_connect_timeout_seconds")]
    pub connect_timeout_seconds: u64,
    #[serde(default = "default_retry_count")]
    pub retry_count: u32,
    #[serde(default)]
    pub proxy_url: Option<String>,
    #[serde(default)]
    pub extra_ca_file: Option<PathBuf>,
    #[serde(default)]
    pub allow_hosts: Vec<String>,
    #[serde(default)]
    pub credentials: serde_json::Value,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            offline: false,
            timeout_seconds: 30,
            connect_timeout_seconds: 10,
            retry_count: 3,
            proxy_url: None,
            extra_ca_file: None,
            allow_hosts: Vec::new(),
            credentials: serde_json::Value::Null,
        }
    }
}

fn default_timeout_seconds() -> u64 {
    30
}
fn default_connect_timeout_seconds() -> u64 {
    10
}
fn default_retry_count() -> u32 {
    3
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ai_provider")]
    pub provider: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub api_key: SecretRef,
    #[serde(default = "default_ai_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_ai_retries")]
    pub max_retries: u32,
    #[serde(default = "default_max_input")]
    pub max_input_characters: usize,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub send_source_code: bool,
    #[serde(default = "default_true")]
    pub redact_secrets: bool,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "openai-compatible".to_string(),
            base_url: None,
            model: None,
            api_key: SecretRef::Environment {
                env: "RUNNERGUARD_AI_API_KEY".to_string(),
            },
            timeout_seconds: 60,
            max_retries: 2,
            max_input_characters: 120_000,
            temperature: 0.0,
            send_source_code: false,
            redact_secrets: true,
        }
    }
}

fn default_ai_provider() -> String {
    "openai-compatible".to_string()
}
fn default_ai_timeout() -> u64 {
    60
}
fn default_ai_retries() -> u32 {
    2
}
fn default_max_input() -> usize {
    120_000
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportConfig {
    #[serde(default = "default_report_formats")]
    pub formats: Vec<String>,
    #[serde(default)]
    pub include_absolute_paths: bool,
    #[serde(default = "default_true")]
    pub include_flow_json_links: bool,
    #[serde(default = "default_true")]
    pub include_ai_section: bool,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            formats: vec!["markdown".to_string(), "html".to_string()],
            include_absolute_paths: false,
            include_flow_json_links: true,
            include_ai_section: true,
        }
    }
}

fn default_report_formats() -> Vec<String> {
    vec!["markdown".to_string(), "html".to_string()]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub file: Option<PathBuf>,
    #[serde(default)]
    pub json: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
            json: false,
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

impl AppConfig {
    /// Merge another config on top of this one. Settings in `other` win
    /// for fields that are "set" (non-default for scalars, non-empty for
    /// collections). Used by `--config` flag combinations.
    pub fn merged_with(mut self, other: AppConfig) -> Self {
        if other.scan.default_rules.is_some() {
            self.scan.default_rules = other.scan.default_rules;
        }
        if !other.scan.output_directory.as_os_str().is_empty() {
            self.scan.output_directory = other.scan.output_directory;
        }
        if other.scan.include_tests {
            self.scan.include_tests = true;
        }
        if !other.scan.continue_on_parse_error {
            self.scan.continue_on_parse_error = false;
        }
        if other.scan.fail_on != self.scan.fail_on {
            self.scan.fail_on = other.scan.fail_on;
        }
        if !other.scan.write_flow_json {
            self.scan.write_flow_json = false;
        }
        if !other.ai.enabled && self.ai.enabled {
            self.ai.enabled = false;
        }
        self
    }

    /// Build a [`RuleFilter`] from the on-disk defaults. CLI flags layer on
    /// top of this in `runnerguard-core`.
    pub fn default_rule_filter(&self) -> RuleFilter {
        RuleFilter::default()
    }
}
