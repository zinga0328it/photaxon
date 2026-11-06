# Rust FiberSpider Service

## Purpose

Own the RAGNO Rust API and its domain adapters. Keep HTTP, configuration,
authentication, cabinet models, PostgreSQL access, and future topology/OpenStack
adapters in separate modules. `main.rs` only loads configuration, initializes
logging, creates the DB pool/router, and serves the API.

## Authorized Files

- `Cargo.toml`, `Cargo.lock`, `src/**`, `photaxon-api.service`.
- Runtime credentials only in the ignored parent `secrets/` directory; never
  stage or print them.

## Client Authentication

- Keep `AuthTokens` API/admin/internal roles independent from scoped clients.
- `CredentialProvider` returns a `ClientPrincipal` with client ID, key ID and
  scopes; never pass the raw token into a domain or database module.
- `EnvCredentialProvider` is replaceable, hashes local tokens in memory, and
  accepts only ACTIVE or unexpired GRACE keys. Provider errors fail closed.
- ALEX is `alex-web` / `dashboard`, limited to the five documented GET routes.
  Do not grant ALEX writes, admin, MCP, shell, SQL or secrets.
- ZINGA is a future separate broker server. Do not implement its network client
  or event bus until the versioned contracts and security review are approved.
- Key metadata uses PENDING, ACTIVE, GRACE, REVOKED and EXPIRED; tests must
  cover dual-key rotation, revocation, expiry, provider outage and scope denial.
- ZINGA is an independent future server, never an in-process dependency or a
  route around RAGNO validation. Do not add a Zinga provider/event client until
  its versioned protocol, replay/idempotency checks and audit contract are
  reviewed.

## Allowed Commands

- `cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings`.
- Start the binary manually only when the Yggdrasil bind address is active.
- Read-only PostgreSQL checks using the protected runtime environment.

## Forbidden Commands

- No Codex daemon, cron, timer, shell/system changes, network/firewall changes,
  Apache/PHP/Nginx setup, direct OpenStack writes, or production DB writes.
- Never bind to `0.0.0.0`, public interfaces, or a non-Yggdrasil address.
- Never put credentials in Rust, tests, logs, command arguments, or Git.

## Backup, Test, Apply, Verify, Rollback

- Back up any existing file before replacing it; do not overwrite user secrets.
- Run `cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings`, and
  `git diff --check` before publish.
- Install the systemd unit only through the approved host configuration workflow.
- Verify listener binding, bearer-token role separation, PostgreSQL health, and
  SSH/Yggdrasil/nftables status after deployment.
- Roll back only this service/unit and its files; do not alter global firewall,
  routes, or other services.

## Done Criteria

- Configuration rejects missing/weak secrets and non-Yggdrasil bind/DB hosts.
- API/admin/internal bearer roles are separated and compared in constant time.
- Database adapters return persisted FiberSpider state without exposing errors
  or credentials.
- Tests pass, secrets remain ignored, and no unrelated host service changes.