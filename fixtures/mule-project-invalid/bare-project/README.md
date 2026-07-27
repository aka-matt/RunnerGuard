# mule-project-invalid/bare-project

Project that is missing both `pom.xml` and `mule-artifact.json`. The
parser still runs, but the `MULE-PROJ-001` rule must fire with
`required-files-exist` failing on `pom.xml`, `mule-artifact.json`, and
also `src/main/mule` once the flow XML is removed.

## Expected findings

- `MULE-PROJ-001` (error) — required files missing.
- `MULE-001` (parser diagnostic) — no Mule XML files under `src/main/mule`.
