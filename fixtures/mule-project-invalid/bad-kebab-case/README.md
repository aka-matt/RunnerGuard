# mule-project-invalid/bad-kebab-case

Project with a flow name that violates the kebab-case rule. The flow is
also large enough to make the size rule optional in this fixture
(it doesn't exceed 25 components, so `MULE-FLOW-002` is silent).

## Expected findings

- `MULE-FLOW-001` (warning) — flow `OrderApiFlow` is not kebab-case.
- `MULE-FLOW-004` (warning) — source-triggered flow without an effective
  error handler.
