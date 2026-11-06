# MCP Control Plane

## Purpose
Expose only explicitly registered FiberSpider tools over stdio. This child process is on-demand and requires an activation/technician context verified against AAA.

## Authorized Files
`src/mcp/**` and the domain tool DTOs. Change DB schema only through reviewed additive migrations under `db/migrations/`.

## Allowed
READ cabinet/topology/ROE/PTE/splitter tools; WRITE_SAFE ONT/anomaly event tools; WRITE_TOPOLOGY cabinet/ROE/PTE/splitter/path proposal tools. All writes must call a typed service and deterministic validator, then persist result and audit atomically.

## Forbidden
No shell, filesystem, SQL arguments, Codex admin tools, systemd, sudo, nftables, SSH, Yggdrasil management, OpenStack CLI, or generic catch-all tool. ADMIN has no registered tools. If a tool is not in the router and permission registry, Codex cannot call it.

## Test and Done
Test tool registration, unknown-tool rejection, schema validation, authorization context, transaction/audit behavior, and no secret in output. `cargo test`, `cargo fmt --check`, and Clippy must pass. Never invoke Codex during development tests.
