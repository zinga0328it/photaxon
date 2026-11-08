# PostgreSQL Adapters

## Purpose
Provide typed repositories, additive migrations, bootstrap seed, and audit persistence for FiberSpider. PostgreSQL on AAA is authoritative.

## Authorized Files
`src/db/**` and `db/migrations/*.sql`. Do not modify cluster files or unrelated node schemas.

## Allowed
Parameterized SQL for named repository operations; transactions for every topology write; idempotent `ON CONFLICT DO NOTHING` seed; sanitized audit fields only.

## Forbidden
Arbitrary SQL input from MCP/Codex, credentials in query logs, destructive DDL, silent mutation of existing topology, direct DB access from Codex, or writes to customer records in tests.

## Backup, Test, Apply, Verify, Rollback
Before live migration/write tests, make an encrypted logical backup. Unit tests use fixtures. User-authorized exception (2026-11-08): AAA write tests may run only within explicit transactions using unique synthetic IDs, prudent lock/statement timeouts and mandatory ROLLBACK. Never modify existing operational rows or customer records, commit synthetic data, or create a test database. Verify absence of every synthetic resource, operation and audit row after rollback. Exercise the Rust domain services, not substitute SQL mutations. Services that commit internally must first expose a transaction-scoped implementation; an outer transaction on another connection cannot undo their commits. Verify migration status and exact IDs/counts. Authentication and backup requirements remain unchanged.

## Done
No duplicate cabinets/lines/splitters/agents; write and audit share one transaction; failed validation leaves no row; secrets are absent from logs/reports.
