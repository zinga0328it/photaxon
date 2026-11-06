# RAGNO to AAA Handoff

Status: verified 2026-11-06
Branch: `pc-ragno`

## Verified

- The RAGNO virtual environment can connect to the remote AAA PostgreSQL node.
- A read-only `SELECT 1` returned successfully.
- `PostgresStore.healthcheck()` succeeded using the private local connection settings.
- Read-only topology checks returned three cabinets, three FTTH lines, three splitters, and three agents.
- The local connection settings are stored only under the ignored `secrets/` directory with directory mode `0700` and file mode `0600`.
- No database rows were changed during these checks.

The host address, database username, and password are intentionally omitted.

## RAGNO Runtime

The primary runtime is the modular Rust Axum API in `rust/`. Its private runtime
configuration is `secrets/runtime.env`; it binds to the configured Yggdrasil
IPv6 address and connects to AAA with a bounded SQLx pool. The legacy Python
FastAPI demo is not the primary runtime.

The enabled `photaxon-api.service` Rust user unit binds only to RAGNO Yggdrasil
port 18000. Authenticated health, OpenStack, cabinet, ROE and topology routes
were all verified HTTP 200. AAA currently returns 3 cabinets, 3 lines, 3
splitters, and 1 ROE associated with CAB-A.

## Current Boundaries

- ALEX is a read-only `alex-web` dashboard client by scope. Lia reports direct
	token-authenticated RAGNO reads working; this RAGNO handoff made no changes
	to ALEX Apache or frontend.
- DevStack `stable/2026.2`, Nova, Glance, Neutron/OVN and four lab servers are
	operational; see the OpenStack report for topology and probe fingerprint.
- PostgreSQL integration tests that write and roll back data were not run.
- Do not merge the `pc-ragno`, `pc-aaa`, and `pc-alex` branches; keep operational secrets and host-specific configuration out of Git.
