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
Before live migration/write tests, make an encrypted logical backup. Unit tests use fixtures; DB integration tests require a dedicated disposable lab DB. Verify migration status and exact IDs/counts. Roll back only newly introduced test IDs or additive objects, never existing resources.

## Done
No duplicate cabinets/lines/splitters/agents; write and audit share one transaction; failed validation leaves no row; secrets are absent from logs/reports.
