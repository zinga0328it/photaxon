# RAGNO Rust API

Status: active on RAGNO Yggdrasil; verified 2026-11-06.

## Routes

All routes below require the API bearer token. ALEX must inject it server-side;
the browser must never receive it.

- `GET /api/health`: authenticated API and PostgreSQL health.
- `GET /api/lab/openstack`: live Keystone, Glance, Nova, Neutron and OVN state.
- `GET /api/lab/cabinets`: AAA cabinet/splitter/ROE data joined with live Nova
  servers, Neutron networks/ports and fresh connectivity evidence.
- `GET /api/lab/roes`: ROEs persisted in AAA.
- `GET /api/lab/topology`: AAA topology plus live cabinet status.

The OpenStack credential is read-only and loaded from the ignored local file
`secrets/openstack-api.env`. The probe snapshot is `secrets/openstack-probes.json`;
it is local, mode 0600, and valid for at most one hour. Nova/Neutron/Glance
resources are still queried live on each request. Responses contain no
credential or OpenStack token.

## Cabinet State

A cabinet is returned as `ACTIVE` only when DB status and Nova server are
ACTIVE, its management and FTTH ports attach to its dedicated ACTIVE Neutron
networks, all CAB-to-CAB tests are blocked, and bidirectional ICMP/TCP
connectivity to CENTRAL-01 passes with fresh evidence. Otherwise its state is
INACTIVE and failed/missing gates are explicit.

The restricted `reader` role can list servers, networks, ports and images. It
cannot list Nova hypervisors/services or read console output, so the OpenStack
response reports `hypervisor_count: null`; an admin CLI check verified one
hypervisor. The API does not claim that reader-only query as a pass.
# RAGNO Rust API

Status: first modular API slice implemented and verified on 2026-11-06.

## Modules

- `rust/src/main.rs`: configuration, logging, DB pool, router, server startup.
- `rust/src/config/`: runtime loading and Yggdrasil/database validation.
- `rust/src/security/`: secret-file loading and constant-time bearer checks.
- `rust/src/api/`: routes, authentication adapters, errors, and health handlers.
- `rust/src/cabinet/`: persisted cabinet response model.
- `rust/src/db/`: PostgreSQL pool and read-only cabinet repository.

## Runtime and Authentication

The API reads `secrets/runtime.env`; it refuses a missing file, empty required
settings, weak/duplicate API tokens, a non-Yggdrasil bind address, or a
non-Yggdrasil DB host. API, admin, and internal token hashes are kept in process
memory and compared in constant time. No token is logged or returned.

- `GET /health`: API bearer token; performs `SELECT 1`.
- `GET /api/v1/cabinets`: API bearer token; reads cabinets from AAA.
- `GET /api/v1/admin/database`: admin bearer token; performs `SELECT 1`.
- `GET /api/v1/internal/cabinets`: internal bearer token; reads cabinets.

The service unit template is `rust/photaxon-api.service`. It has not been
installed/enabled. Firewall access for remote Yggdrasil peers has not been
applied; no nftables rules were changed. The temporary server used in tests was
stopped. Do not treat this slice as a persistent deployment yet.

## Verified Results

- `cargo test --manifest-path rust/Cargo.toml --locked`: 3 passed.
- `cargo fmt --manifest-path rust/Cargo.toml -- --check`: passed.
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`:
  passed.
- Live HTTP test on the configured Yggdrasil socket: missing token 401, wrong
  token 401, API health 200, API cabinet read 200 with 3 records, API token on
  admin route 401, admin token on admin route 200, internal token on internal
  cabinet route 200.
- `ss` confirmed the listener bound to the configured Yggdrasil IPv6 and port,
  not a wildcard. This local test does not prove remote-peer firewall reachability.
- PostgreSQL `SELECT 1` and cabinet reads succeeded. No DB rows were modified.

## Not Implemented

This slice does not yet include technician/Telegram OTP, activation sessions,
Codex on-demand, topology mutation/audit, OpenStack SDK/Neutron, Neutron CAB
networks, or the dashboard. Codex was not invoked. The Python activation API
remains legacy and is not replaced yet.