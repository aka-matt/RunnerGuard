//! Construct an [`AiService`] from the CLI's loaded config and CLI
//! flags.
//!
//! The builder returns `Ok(None)` for every "we deliberately won't run
//! AI" condition (offline mode, AI disabled in config, missing key,
//! unknown provider, prompt templates missing on disk). The CLI
//! surfaces those as `tracing::info!` lines so the user knows why AI
//! is silently skipping — we don't want them staring at a "→ invoking
//! AI" prompt that never finishes.
//!
//! The hard, recoverable failures (provider construction error,
//! prompt-load error) return `Err` with a one-line message; the CLI
//! records that as an `AI-001` diagnostic via the progress sink and
//! continues with the deterministic report — per the design contract
//! "AI failure must never invalidate the deterministic report".
//!
//! Notes:
//!
//! * The schemas live at the workspace root. We `include_str!` them at
//!   compile time so the binary doesn't need to ship them next to it.
//!   Tests in `runnerguard-ai/tests/ai.rs` already do the same.
//! * The prompt templates live in `prompts/` at the workspace root
//!   too. We resolve that path at runtime via `env!("CARGO_MANIFEST_DIR")`.
//! * The AI service only kicks in when *both* the CLI flag
//!   (`--ai optional|required`) and the YAML config (`ai.enabled: true`)
//!   are set. Either alone is a no-op.

use anyhow::{Context, Result};
use runnerguard_ai::{AiService, OpenAiCompatibleProvider};
use runnerguard_config::{AiConfig, NetworkConfig};
use runnerguard_http::{HttpClientConfig, ReqwestHttpClient};
use runnerguard_model::SecretRef;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// The JSON schema the provider is asked to emit. Mirrors
/// `schemas/ai-response.schema.json`; pinned at compile time so the
/// AI response parser and the AI provider can never drift.
const AI_RESPONSE_SCHEMA: &str = include_str!("../../../schemas/ai-response.schema.json");

/// Path to the bundled prompt templates (`prompts/` at the workspace
/// root), resolved at compile time relative to the CLI manifest.
fn prompts_dir() -> std::path::PathBuf {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .map(|p| p.join("prompts"))
        .unwrap_or_else(|| manifest.join("../../prompts"))
}

/// Reasons the AI pass is intentionally skipped. We surface these via
/// `tracing::info!` so a user running `--ai optional` against an
/// unconfigured box doesn't think the binary hung — the AI pass
/// simply isn't going to happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiSkipped {
    /// `ai.enabled` is `false` in the YAML config.
    DisabledInConfig,
    /// `network.offline` is `true` and AI requires network.
    Offline,
    /// `ai.provider` is something other than `openai-compatible`.
    UnknownProvider(&'static str),
    /// `ai.api_key` is empty (`SecretRef::Empty`).
    NoApiKey,
}

impl AiSkipped {
    pub fn message(self) -> String {
        match self {
            Self::DisabledInConfig => "AI pass skipped (ai.enabled is false in config)".to_string(),
            Self::Offline => "AI pass skipped (network.offline is true)".to_string(),
            Self::UnknownProvider(p) => {
                format!(
                    "AI pass skipped (unknown provider `{p}`; only `openai-compatible` is wired)"
                )
            }
            Self::NoApiKey => "AI pass skipped (no api_key configured)".to_string(),
        }
    }
}

/// Try to build an AI service. On hard failures (we attempted to build
/// one but couldn't) returns `Err`; on deliberate skips returns
/// `Ok(None)`.
pub fn try_build(
    cfg: &AiConfig,
    net: &NetworkConfig,
) -> Result<Option<Arc<AiService>>, anyhow::Error> {
    if !cfg.enabled {
        tracing::info!("{}", AiSkipped::DisabledInConfig.message());
        return Ok(None);
    }
    if net.offline {
        tracing::info!("{}", AiSkipped::Offline.message());
        return Ok(None);
    }
    if cfg.provider != "openai-compatible" {
        tracing::info!(
            "{}",
            AiSkipped::UnknownProvider("not_openai_compatible").message()
        );
        return Ok(None);
    }
    if matches!(cfg.api_key, SecretRef::Empty) {
        tracing::info!("{}", AiSkipped::NoApiKey.message());
        return Ok(None);
    }

    // Load bundled prompt templates. This is a recoverable error:
    // the binary still runs, the scan still completes, the deterministic
    // report is unaffected — AI just won't run this time.
    let prompts = prompts_dir();
    runnerguard_ai::prompt::load_bundled(&prompts)
        .with_context(|| format!("loading bundled prompts from {}", prompts.display()))?;

    let http_cfg = HttpClientConfig {
        offline: net.offline,
        timeout: Duration::from_secs(net.timeout_seconds),
        connect_timeout: Duration::from_secs(net.connect_timeout_seconds),
        retry_count: net.retry_count,
        max_response_bytes: 10 * 1024 * 1024,
        proxy_url: net.proxy_url.clone(),
        extra_ca_file: net.extra_ca_file.as_ref().map(|p| p.display().to_string()),
        allow_hosts: net.allow_hosts.clone(),
        user_agent: format!("runnerguard/{}", env!("CARGO_PKG_VERSION")),
    };
    let http = Arc::new(
        ReqwestHttpClient::new(http_cfg).context("building reqwest HTTP client for AI provider")?,
    );

    let base_url = cfg
        .base_url
        .clone()
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    let model = cfg
        .model
        .clone()
        .unwrap_or_else(|| "gpt-4o-mini".to_string());

    let provider = OpenAiCompatibleProvider::new(
        "openai-compatible",
        base_url,
        model,
        Some(cfg.api_key.clone()),
        http,
    )
    .with_timeout(Duration::from_secs(cfg.timeout_seconds.max(1)));

    let service = Arc::new(AiService::new(
        Arc::new(provider),
        AI_RESPONSE_SCHEMA.to_string(),
    ));
    Ok(Some(service))
}

/// Helper used by tests / examples to point the prompt loader at an
/// explicit directory instead of the workspace default.
#[allow(dead_code)]
pub fn load_bundled_prompts(dir: &Path) -> Result<()> {
    runnerguard_ai::prompt::load_bundled(dir)
        .context("loading bundled prompts")
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use runnerguard_config::{AiConfig, NetworkConfig};

    fn base_ai() -> AiConfig {
        AiConfig {
            enabled: true,
            ..AiConfig::default()
        }
    }

    fn base_net() -> NetworkConfig {
        NetworkConfig::default()
    }

    #[test]
    fn skips_when_disabled_in_config() {
        let ai = AiConfig {
            enabled: false,
            ..base_ai()
        };
        let net = base_net();
        let res = try_build(&ai, &net).expect("disabled should not error");
        assert!(res.is_none(), "expected deliberate skip, got service");
    }

    #[test]
    fn skips_when_offline() {
        let ai = base_ai();
        let net = NetworkConfig {
            offline: true,
            ..base_net()
        };
        let res = try_build(&ai, &net).expect("offline should not error");
        assert!(res.is_none(), "expected offline skip, got service");
    }

    #[test]
    fn skips_when_no_api_key() {
        let ai = AiConfig {
            enabled: true,
            api_key: SecretRef::Empty,
            ..base_ai()
        };
        let net = base_net();
        let res = try_build(&ai, &net).expect("missing key should not error");
        assert!(res.is_none(), "expected missing-key skip, got service");
    }

    #[test]
    fn skips_on_unknown_provider() {
        let ai = AiConfig {
            enabled: true,
            provider: "anthropic".to_string(),
            ..base_ai()
        };
        let net = base_net();
        let res = try_build(&ai, &net).expect("unknown provider should not error");
        assert!(res.is_none(), "expected unknown-provider skip, got service");
    }

    #[test]
    fn ai_skipped_messages_are_distinct() {
        // Each skip reason gives the user a different hint; guards against
        // accidentally collapsing them to the same text.
        let msgs = [
            AiSkipped::DisabledInConfig.message(),
            AiSkipped::Offline.message(),
            AiSkipped::UnknownProvider("x").message(),
            AiSkipped::NoApiKey.message(),
        ];
        let uniq: std::collections::BTreeSet<_> = msgs.iter().collect();
        assert_eq!(uniq.len(), msgs.len(), "messages must be unique");
    }
}
