# Photaxon RAGNO Node

This directory is the RAGNO working copy of the `pc-ragno` branch. Keep the
`pc-ragno`, `pc-aaa`, and `pc-alex` node branches separate. Do not deploy this
backend into a public web root. This Git branch is a curated public snapshot:
publish only sanitized code and documentation, never credentials, operational
configuration, database dumps, or host-specific secrets.

## Autonomous Workflow

- Read this file and inspect the working tree before changing files. Preserve
	existing user changes.
- Proceed autonomously with scoped code, documentation, virtualenv, and test
	work. Run focused validation after each change and update this file when
	operational steps change.
- Before privileged work, check `sudo -n true`. If it succeeds, use `sudo -n`.
	If it fails, collect the required root operations and ask once for the user
	to authenticate with `sudo -v` in the same terminal session; then recheck.
	Never ask for passwords in chat or store them in scripts, shell history,
	tracked files, or logs. Keep machine-local credentials only in ignored
	`secrets/` files with restrictive permissions.
- Keep database reads read-only by default. Run write/integration tests only
	against a dedicated disposable lab database, never production data.
- Back up and test offline before destructive changes. Do not reboot, change
	the kernel, firewall, SSH, Apache, system services, host networking, or
	install/configure OpenStack without an explicit request for that operation.
- For an explicit Git publish request, review the complete diff, exclude all
	secrets and machine-specific settings, and publish only scoped changes on
	`pc-ragno`. Never force-push or merge node branches.

## Runtime Setup

Rust is the primary application runtime. Use the stable toolchain and the
modular crate under `rust/`:

```sh
PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path rust/Cargo.toml --locked
PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path rust/Cargo.toml --release
```

The Python API and virtualenv are legacy/demo tooling. Do not add new FiberSpider
runtime features to Python; keep changes in Rust unless a narrowly scoped
maintenance or migration task explicitly needs Python.

## Rust API

The Rust API loads `secrets/runtime.env` via `dotenvy`, validates that both API
bind and DB host are Yggdrasil IPv6 addresses, and refuses missing credentials.
Its listener must bind only to the configured Yggdrasil address. Run locally:

```sh
PATH="$HOME/.cargo/bin:$PATH" FIBERSPIDER_ENV_FILE=/home/ragno/apps/photaxon/secrets/runtime.env cargo run --manifest-path rust/Cargo.toml --release
```

Routes use distinct legacy API/admin/internal credentials plus the replaceable
`CredentialProvider` boundary for client identity. ALEX is `alex-web` with only
the `dashboard` scope on five GET routes. Tokens are never logged or returned.
The API runs as the enabled user unit `photaxon-api.service`, bound only to the
configured Yggdrasil address. Its template is `rust/photaxon-api.service`.

RAGNO has no Apache/PHP/NGINX role. Do not proxy through Apache locally; ALEX
may reverse-proxy to the RAGNO Yggdrasil listener after its own configuration is
authorized. The existing Python demo is not the primary API.

See `docs/RAGNO-AAA-STATUS.md` for the sanitized handoff status for Lia.

## Client Identity and Future ZINGA

- `CredentialProvider` is the replaceable local boundary; `EnvCredentialProvider`
	stores only token hashes and issues a `ClientPrincipal` without the presented
	token. Runtime secret: ignored `secrets/runtime.env`, mode 0600.
- ALEX is a read-only dashboard client. Its token cannot reach admin/internal,
	write, MCP, or secret routes. New scopes require tests and documentation.
- ZINGA is a future separate server/broker, never an internal RAGNO library.
	It may manage service identity, key lifecycle and broker events, but cannot
	access AAA SQL, shell, Linux administration, or MCP admin capabilities.
- Codex remains reasoning through authorized MCP. Every write goes through the
	Rust validator and audited domain service. No credential provider outage may
	trigger anonymous access or local key generation.
- See `docs/AUTH-CLIENTS-AND-SCOPES.md`, `docs/ALEX-RAGNO-YGGDRASIL.md`, and
	`docs/ZINGA-BROKER-INTEGRATION.md` for the trust boundaries and contracts.

## Secrets, API Keys, and Rust

- The Telegram token file is `secrets/telegram-bot.env` relative to the
	repository root. `agent/demo_api.py` reads it through `TELEGRAM_SECRET_PATH`,
	whose default is that path. The file is local-only and ignored by Git.
- If the Telegram file is absent, create it from the repository root in a
	terminal. This prompts without echoing the token and refuses to overwrite an
	existing file:

	```sh
	mkdir -p secrets && chmod 700 secrets
	umask 077
	read -r -s -p 'Telegram bot token: ' token; printf '\n'
	(set -o noclobber; printf 'TELEGRAM_BOT_TOKEN=%s\n' "$token" > secrets/telegram-bot.env)
	unset token
	chmod 600 secrets/telegram-bot.env
	```

- Never put Telegram, OpenStack, database, or API credentials in Python/Rust
	source, command arguments, tracked `.env` files, API responses, logs, test
	fixtures, or GitHub issues. Keep each machine credential in an ignored file
	under `secrets/`, directory mode `0700`, file mode `0600`.
- Generate machine secrets only in the terminal, writing directly to a new
	protected file (for example, `umask 077; openssl rand -hex 32 >
	secrets/<service>-api-key; chmod 600 secrets/<service>-api-key`). Never print
	the generated value or paste it into chat. Do not generate replacement DB or
	Telegram credentials unless explicitly required; obtain those from their
	owning service.
- Human account passwords must be salted and hashed with Argon2id; never store
	or log plaintext passwords. Machine API keys must be high entropy, scoped per
	service, rotatable, verified without logging their value, and stored as a
	verifier/hash rather than plaintext in the database.
- For Rust services, use the stable toolchain, commit `Cargo.lock` for
	applications, validate configuration at startup, return errors instead of
	panicking on runtime input, and keep secrets out of `Debug` output. Bind the
	RAGNO API only to its Yggdrasil IPv6 address; never bind to `0.0.0.0` or a
	public interface. Keep Python activation code as legacy until a reviewed Rust
	parity/migration plan replaces it; don't add new FiberSpider behavior there.
- Before Rust changes are published, run `cargo test`, `cargo clippy -- -D
	warnings`, `cargo audit`, and a secret scan. Never run these tests against
	production data.

## OpenStack and Neutron

- The authorized RAGNO target is DevStack `stable/2026.2`, not `master`, run
	by a dedicated non-root `stack` user under `/opt/stack`.
- Precheck and document host resources and active services before installing.
	Label any resource-constrained deployment accurately; never claim an
	unverified component is operational.
- Keep Neutron networks tenant-private. Do not bind provider networks to
	physical/Yggdrasil interfaces, add public access, change the default route,
	or manually alter nftables, SSH, Apache, or Yggdrasil to make DevStack pass.
- Do not add host `NOPASSWD: ALL` unless the user explicitly authorizes that
	specific privilege change; prefer the narrowest workable sudo setup.
- The Rust OpenStack adapter is read-only and uses a restricted application
	credential from ignored `secrets/openstack-api.env`. DevStack `stable/2026.2`
	and the isolated CirrOS Neutron lab are verified on RAGNO. Keep mutations in
	reviewed operator commands, never in the API.
- Probe evidence is local-only in ignored `secrets/openstack-probes.json`, mode
	0600, and bound to a canonical topology fingerprint. Age is informational;
	any relevant topology mismatch or invalid probe returns DEGRADED and prevents
	ACTIVE cabinet status. Never alter the original guest-test timestamp/results.
- The shipped guest probes are cloud-init user-data and ran at initial boot.
	Their PASS remains usable only while the live topology fingerprint matches.
	On mismatch, do not edit the fingerprint or timestamp to force PASS: a new
	real guest test is required. There is currently no safe guest execution
	channel (no keypair/tenant route, serial disabled, loopback VNC inactive).
- See `openstack/PRECHECK.md`, `docs/OPENSTACK-RAGNO.md`,
	`docs/NEUTRON-TOPOLOGY.md`, and `docs/OPENSTACK-TEST-REPORT.md` for status.

## Tests

Run the unittest suite (PostgreSQL integration tests are skipped by default):

```sh
PYTHONDONTWRITEBYTECODE=1 .venv/bin/python -m unittest discover -s tests -v
```

The agent checks are standalone scripts rather than unittest-discoverable tests:

```sh
.venv/bin/python -m agent.test_otp_service
.venv/bin/python -m agent.test_technician_auth
.venv/bin/python -m agent.test_telegram_otp_auth
.venv/bin/python -m agent.test_dry_run
.venv/bin/python -m agent.test_simulated_ont_detected
.venv/bin/python -m agent.test_dry_run_provisioning_close
.venv/bin/python -m agent.test_llm_runtime
.venv/bin/python agent/test_dry_run_field_path.py parse_test
.venv/bin/python agent/test_dry_run_field_path.py classification_test
```

Do not run `agent/test_real_telegram_provider.py` or
`agent/test_real_telegram_otp.py` as routine tests; they can contact Telegram
and send messages. `agent/test_dry_run_multi_agent.py` and the default mode of
`agent/test_dry_run_field_path.py` invoke Codex. PostgreSQL integration tests
require `FIBERSPIDER_RUN_DB_TESTS=1` and a dedicated disposable lab database;
never point them at production data.
