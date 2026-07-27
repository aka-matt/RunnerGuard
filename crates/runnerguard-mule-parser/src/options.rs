//! Options that control how a project is parsed.

use runnerguard_fs::Limits;

#[derive(Debug, Clone)]
pub struct ParseOptions {
    pub limits: Limits,
    pub continue_on_parse_error: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            continue_on_parse_error: true,
        }
    }
}
