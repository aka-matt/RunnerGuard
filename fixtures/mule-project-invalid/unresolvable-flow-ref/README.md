# mule-project-invalid/unresolvable-flow-ref

Project where one flow references a sub-flow that does not exist. The
`MULE-FLOW-003` rule must fire on the caller.

## Expected findings

- `MULE-FLOW-003` (error) — flow `order-api-flow` references
  `validate-order-subflow`, which is not defined.
