# mule-project-invalid/missing-mule-config

Project whose `mule-artifact.json` is missing the required `minMuleVersion`
and `bundleEngineVersion`. Existence of the file is fine — this is a model
validation failure (handled by the config parser, not the rule engine).

## Expected findings

- Config layer diagnostic: `MissingKeys { required: ["minMuleVersion", "bundleEngineVersion"] }`.
- `MULE-PROJ-001` (warning) — required files are present, so the rule does
  not fire on the file list; the model just won't load.
