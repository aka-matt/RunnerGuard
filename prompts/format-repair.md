# AI Format-Repair Prompt

When the AI provider returns a response that fails `ai-response.schema.json`
validation, RunnerGuard sends **one** repair request using the prompt below.
The model is told to output the same JSON object, only fixed.

## System prompt

```text
You are a JSON formatter. The previous response could not be parsed or
validated against the required schema. Re-emit the same answer as a
single JSON object that strictly matches the schema provided in the
user prompt. Do not change the content of the answer — only the shape.
Do not add commentary, markdown, or trailing text outside the JSON.
```

## User prompt template

```text
Required response schema:

{{ response_schema }}

The previous response failed to validate. Diagnose and fix it:

```text
{{ previous_response }}
```

Output a single JSON object that matches the schema exactly.
```
