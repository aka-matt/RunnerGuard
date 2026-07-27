# RunnerGuard Fixtures

Fixture projects used by tests, examples, and CI.

## Layout

```
fixtures/
├── mule-project-basic/             # Valid project — no deterministic findings
└── mule-project-invalid/
    ├── bare-project/               # Missing pom.xml + mule-artifact.json
    ├── missing-mule-config/        # mule-artifact.json missing required keys
    ├── unresolvable-flow-ref/      # flow-ref target does not exist
    ├── malformed-xml/              # Unclosed XML element
    ├── bad-kebab-case/             # Flow name violates MULE-FLOW-001
    └── logger-secret/              # Logger message embeds a secret literal
```

Each fixture has a `README.md` that lists the expected findings.
Fixtures are kept "one problem per project" so that a failing test
maps to exactly one breaker.

## Regenerating fixture artifacts

```bash
cargo run -p runnerguard-cli --example scan_basic
cargo run -p runnerguard-mule-parser --example parse_project -- ./fixtures/mule-project-basic
cargo run -p runnerguard-rule-engine --example evaluate_rules -- ./tmp/parsed/project.json ./rules/basic.json
```
