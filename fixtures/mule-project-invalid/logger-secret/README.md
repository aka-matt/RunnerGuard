# mule-project-invalid/logger-secret

Project with a logger component whose message embeds what looks like a
literal API key. The `MULE-LOG-001` rule must fire at *critical* severity.

## Expected findings

- `MULE-LOG-001` (critical) — `password=hunter2` matches the secret regex.
- `MULE-HTTP-001` (error) — hard-coded port also present.
