# FiberSpider Policy

## Purpose
Own deterministic authorization and topology/input validation.

## Authorized Files
`src/policy/**` and policy-only unit tests.

## Allowed
Pure checks for tool classes, identifiers, capacity limits, resource links, and safe test-event content.

## Forbidden
Database writes, shell/Codex calls, changing permissions at runtime, or making LLM output authoritative.

## Test and Done
Invalid inputs reject before DB access; no credentials appear in errors; every write tool has a class and ADMIN has no Codex-exposed route.
