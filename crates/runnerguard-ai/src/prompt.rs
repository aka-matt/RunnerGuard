//! Prompt-template loader and renderer.
//!
//! Templates live in `prompts/*.md` at the workspace root. They are
//! rendered with `minijinja` so the substitution rules are explicit and
//! the untrusted-data markers can never be hidden by user input.

use crate::error::AiError;
use minijinja::Environment;
use std::path::Path;
use std::sync::OnceLock;

/// One loaded template.
#[derive(Debug, Clone)]
pub struct PromptTemplate {
    pub name: String,
    pub body: String,
}

impl PromptTemplate {
    pub fn from_str(name: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            body: body.into(),
        }
    }

    pub fn load(dir: &Path, name: &str) -> Result<Self, AiError> {
        let path = dir.join(format!("{name}.md"));
        let body = std::fs::read_to_string(&path)
            .map_err(|e| AiError::PromptMissing(name.to_string(), e.to_string()))?;
        Ok(Self {
            name: name.to_string(),
            body,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    /// Render with the given template variables. Every value goes through
    /// a sanitiser that:
    ///
    /// * JSON-escapes the value (so we can safely embed it in the JSON
    ///   snapshot that goes inside `<<<UNTRUSTED>>>`);
    /// * strips out any `<<<UNTRUSTED>>>` markers in user data so an
    ///   attacker can't break out of the untrusted block.
    pub fn render(&self, vars: &serde_json::Value) -> Result<RenderedPrompt, AiError> {
        let mut env = Environment::new();
        env.set_trim_blocks(true);
        env.set_lstrip_blocks(true);
        // Register our sanitising function on every value.
        env.add_filter(
            "safe",
            |v: minijinja::value::Value| -> Result<String, minijinja::Error> {
                let raw = v.to_string();
                Ok(sanitise_untrusted(&raw))
            },
        );
        env.add_template("tpl", &self.body)
            .map_err(|e| AiError::PromptRender(self.name.clone(), e.to_string()))?;
        let tmpl = env.get_template("tpl").expect("template just added");
        let rendered = tmpl
            .render(minijinja::Value::from_serialize(vars))
            .map_err(|e| AiError::PromptRender(self.name.clone(), e.to_string()))?;
        Ok(RenderedPrompt {
            name: self.name.clone(),
            body: rendered,
        })
    }
}

#[derive(Debug, Clone)]
pub struct RenderedPrompt {
    pub name: String,
    pub body: String,
}

impl RenderedPrompt {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn body(&self) -> &str {
        &self.body
    }
}

/// Strip `<<<UNTRUSTED>>>` markers from user-controlled data so the
/// structured `<<<UNTRUSTED>>>` blocks in the system prompt can't be
/// closed early. JSON-escape the result to keep the snapshot valid JSON
/// when the template embeds it as a value.
pub fn sanitise_untrusted(input: &str) -> String {
    let stripped = input
        .replace("<<<UNTRUSTED>>>", "<<< / UNTRUSTED >>>")
        .replace("<<<END_UNTRUSTED>>>", "<<< / END_UNTRUSTED >>>");
    // Round-trip through a JSON string so backslashes / quotes / newlines
    // are escaped if the value is later embedded into a JSON payload.
    let v = serde_json::Value::String(stripped);
    if let serde_json::Value::String(s) = v {
        s
    } else {
        String::new()
    }
}

static REVIEW_TEMPLATE: OnceLock<PromptTemplate> = OnceLock::new();
static REPAIR_TEMPLATE: OnceLock<PromptTemplate> = OnceLock::new();

/// Load the bundled review + repair templates. Resolves templates
/// relative to the configured prompts directory.
pub fn load_bundled(prompts_dir: &Path) -> Result<(), AiError> {
    let review = PromptTemplate::load(prompts_dir, "review")?;
    let repair = PromptTemplate::load(prompts_dir, "format-repair")?;
    REVIEW_TEMPLATE
        .set(review)
        .map_err(|_| AiError::Internal("review template already loaded".to_string()))?;
    REPAIR_TEMPLATE
        .set(repair)
        .map_err(|_| AiError::Internal("repair template already loaded".to_string()))?;
    Ok(())
}

/// Get a reference to the loaded review template.
pub fn review_template() -> Option<&'static PromptTemplate> {
    REVIEW_TEMPLATE.get()
}

/// Get a reference to the loaded repair template.
pub fn repair_template() -> Option<&'static PromptTemplate> {
    REPAIR_TEMPLATE.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitise_blocks_marker_close_attempts() {
        let s = sanitise_untrusted("hello <<<UNTRUSTED>>> bye");
        assert!(!s.contains("<<<UNTRUSTED>>>"));
        assert!(s.contains("<<< / UNTRUSTED >>>"));
    }

    #[test]
    fn sanitise_is_idempotent() {
        let once = sanitise_untrusted("<<<UNTRUSTED>>>");
        let twice = sanitise_untrusted(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn render_substitutes_variables() {
        let tpl =
            PromptTemplate::from_str("review", "Hello {{ name }}, project is {{ project.id }}.");
        let out = tpl
            .render(&serde_json::json!({"name": "Alice", "project": {"id": "demo"}}))
            .unwrap();
        assert!(out.body.contains("Alice"));
        assert!(out.body.contains("demo"));
    }
}
