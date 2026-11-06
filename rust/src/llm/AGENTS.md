# On-Demand LLM Runtime

## Purpose
Future module for a single Codex verification during a verified technician activation. Codex is a reasoning engine, not a server administrator.

## Authorized Files
`src/llm/**` and mock-only tests. No Codex service, timer, cron, or boot hook.

## Allowed
Spawn one bounded `codex exec` with an explicit read-only sandbox and a single prompt after Rust confirms ACTIVE technician + OTP. Expose only registered MCP tools. Validate all structured output deterministically before persistence.

## Forbidden
Calling Codex during development, startup, polling, or normal health checks; direct DB/filesystem/shell/system access; tool discovery beyond the Rust allowlist; infrastructure changes; logging prompts containing secrets.

## Test and Done
Use a mock process by default; test timeout, malformed output, denial of unregistered tools, and termination. A real invocation is allowed only within the OTP-verified activation workflow and must be audited without secrets.
