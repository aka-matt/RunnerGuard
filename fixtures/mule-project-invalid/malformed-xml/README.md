# mule-project-invalid/malformed-xml

Project whose Mule XML is structurally broken (unclosed element). The
parser must surface a parser-level diagnostic (`XML-001`) and continue
with the rest of the project if `continue_on_parse_error` is true.

## Expected findings

- `XML-001` (error) — parser-level diagnostic for the malformed file.
- `MULE-001` (error) — no usable Mule documents, so the parser reports the
  project as having zero XML files.
