# RunnerGuard

> A Rust 2024 CLI/TUI that statically validates MuleSoft 4 projects against a JSON rule DSL.

RunnerGuard reads a MuleSoft project, parses the XML into a structured
flow model, evaluates a JSON rule pack against it, and produces
Markdown + HTML reports. The deterministic rule engine is the source of
truth; an optional AI pass suggests additional findings, never
replacing deterministic ones.

## Highlights

- **12-crate Cargo workspace** with a single published binary
  (`runnerguard`).
- **Deterministic-first rule engine** — operators are a closed whitelist
  (no arbitrary expressions in phase 1) and every rule-compile error is
  surfaced.
- **Secret-safe core** — `SecretRef` debug redaction, header
  redaction in the HTTP client, no raw API keys in panic chains.
- **Offline-ready** — `offline: true` disables the network, the AI
  provider, and remote rule servers.
- **Two front-ends** — `runnerguard scan` (CLI) and `runnerguard tui`
  (Ratatui) backing the same `ScanService`.

## Quick start

```bash
# Build the CLI
cargo build --release -p runnerguard-cli --bin runnerguard

# Scan a Mule project
./target/release/runnerguard scan ./fixtures/mule-project-basic \
    --config ./runnerguard.yaml \
    --rules ./rules/basic.json \
    --output ./scan-output

# Launch the TUI
./target/release/runnerguard tui ./fixtures/mule-project-basic
```

## Workspace layout

```text
apps/runnerguard-cli/            # published binary: `runnerguard`
crates/
  runnerguard-model/             # shared serde models, zero I/O
  runnerguard-json/              # JSON read/write + Schema validation
  runnerguard-config/            # YAML config, SecretRef, paths
  runnerguard-fs/                # project discovery, atomic writes
  runnerguard-http/              # generic HTTP client (retry, allowlist)
  runnerguard-ai/                # OpenAI-compatible provider + parsing
  runnerguard-mule-parser/       # namespace-aware XML → flow JSON
  runnerguard-rule-engine/       # fact DSL, operators, finding generation
  runnerguard-report/            # Markdown + HTML renderers
  runnerguard-core/              # orchestration: ScanService + ScanEvent
  runnerguard-tui/               # Ratatui front-end
tools/xtask/                     # developer + release scripts
schemas/                         # JSON Schemas: rule-set, flow, finding, ai-response
rules/                           # shipped rule packs
fixtures/                        # sample Mule projects
prompts/                         # AI prompt templates
templates/                       # report.html
```

## CLI

```text
runnerguard scan <project>    Run a full scan
runnerguard tui   <project>   Open the Ratatui front-end
runnerguard config init       Create a default `runnerguard.yaml`
runnerguard config show       Echo the resolved config (secrets redacted)
runnerguard rules  validate   Check a rule file against the schema
runnerguard rules  compile    Surface all rule-compile errors
runnerguard --help            ...
```

Exit codes follow the spec:

| Code | Meaning                                                    |
| ---: | ---------------------------------------------------------- |
| `0`  | success, threshold not reached                             |
| `1`  | success but findings ≥ `--fail-on`                         |
| `2`  | CLI / config argument error                                |
| `3`  | unparseable input (file / XML / JSON / rules) — not actionable |
| `4`  | forced network / AI operation failed                       |
| `5`  | report write failed                                        |
| `130`| user interrupt                                             |

## Configuration

The default config file is `runnerguard.yaml`. It is searched in this
order:

1. `--config <path>` CLI flag
2. `RUNNERGUARD_CONFIG` env var
3. `./runnerguard.yaml`
4. `./runnerguard/runnerguard.yaml`
5. `$XDG_CONFIG_HOME/runnerguard/runnerguard.yaml`

Secrets are never written to the config file:

```yaml
ai:
  enabled: true
  api_key:
    env: OPENAI_API_KEY
  base_url: https://api.openai.com/v1
  model: gpt-4o-mini
  send_source_code: false
network:
  allow_hosts:
    - api.openai.com
  offline: false
limits:
  max_xml_depth: 32
  max_file_bytes: 10485760
  max_project_files: 5000
```

## Rule DSL

A minimal rule file:

```json
{
  "schema_version": "1.0",
  "rules": [
    {
      "id": "MULE-LOG-001",
      "title": "Logger missing",
      "severity": "warning",
      "target": { "entity": "flow" },
      "when": {
        "all": [
          { "operator": "exists", "path": "flow.components" },
          { "operator": "not-contains-component", "value": "mule:logger" }
        ]
      },
      "message": "Flow {{ flow.name }} is missing a Logger component.",
      "recommendation": "Add a <logger/> component early in the flow."
    }
  ]
}
```

Full operator list lives in `implementation_docs/RunnerGuard__实施文档.md` §6.7.

## Development

```bash
# Workspace-wide lint/test/doc cycle
cargo run -p xtask -- ci

# Or run the individual checks
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test  --workspace
cargo doc   --workspace --no-deps

# Work on a single crate
cargo check -p runnerguard-mule-parser
cargo test  -p runnerguard-rule-engine

# Run an example
cargo run -p runnerguard-tui --example demo
```

### Toolchain

The workspace pins `rust-version = "1.88"` in `Cargo.toml`. The
`rust-toolchain.toml` file pins the **stable** channel; CI exercises
both stable and the MSRV.

### Layer rules

- `runnerguard-model` has zero I/O. Do not add `std::fs`, `tokio`, or
  HTTP to it.
- The CLI/TUI contain zero parsing, rule, or reporting logic. They
  wire user input to `runnerguard-core`.
- `runnerguard-core` does not `println!` or draw — it emits
  `ScanEvent`s to a `ProgressSink`.

## Security notes

- `SecretRef` is tagged `untagged`; the `Debug` impl prints
  `***REDACTED***`. `config show` redacts secrets unconditionally.
- The HTTP client strips `Authorization`, `Proxy-Authorization`,
  `X-API-Key`, `Cookie`, and `Set-Cookie` from debug logs.
- Panic payloads and `anyhow` chains never include raw API keys or
  full request bodies.
- AI is **off** by default. When on, `send_source_code: false` is the
  default — only flow structure, component names, deterministic
  findings, and redacted attributes leave the host.
- All Mule XML / DataWeave / logger messages / comments are treated as
  untrusted input by the AI provider template (the system prompt
  wraps them in `<<<UNTRUSTED>>>` markers).

## Status

Phase 0 → 15 of the implementation plan are committed on
`step2_M3`. CI is on `main`. See
`implementation_docs/RunnerGuard__实施文档.md` for the full design.
