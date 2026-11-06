# Cabinet Domain

## Purpose
Represent stable cabinet identity and its agent-facing state. Existing records CAB-A/B/C are owned by PostgreSQL AAA.

## Authorized Files
`src/cabinet/**` and cabinet-specific domain/service tests.

## Allowed
Typed cabinet models and deterministic state transitions; use repository/service traits for persistence.

## Forbidden
Direct SQL, Codex, OpenStack/Neutron calls, Telegram, shell, or OS/network changes from cabinet models.

## Test and Done
State transitions and identity constraints have unit tests; do not insert duplicate CAB-A/B/C records.
