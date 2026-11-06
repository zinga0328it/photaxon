# RAGNO Final Handoff

Verified 2026-11-06 on branch `pc-ragno`. RAGNO is the FiberSpider Rust control
plane; this handoff contains no credential values or current host address.

## Role and Architecture

- RAGNO: Rust/Axum FTTH control plane on Yggdrasil, API port 18000.
- OpenStack: isolated compute/network lab on RAGNO, read through a restricted
  Keystone application credential.
- AAA: persistent PostgreSQL, reached by RAGNO over Yggdrasil; no ALEX/Codex
  direct database access.
- ALEX: public web edge and read-only dashboard client. Apache/frontend are
  owned by ALEX and were not changed by the RAGNO verification.
- CODEX: on-demand reasoning through registered Rust MCP tools, not a daemon or
  Linux administrator.
- ZINGA: future separate server/broker/key manager; not a RAGNO library and not
  implemented in this phase.

```text
Browser -> ALEX HTTPS/Apache -> Yggdrasil -> RAGNO Rust API
                                      |-> OpenStack read-only APIs
                                      `-> AAA PostgreSQL
CODEX -> authorized MCP -> Rust validator -> domain services -> AAA/audit
```

## API

Rust binds only to RAGNO's configured Yggdrasil IPv6 address on port 18000.
Verified authenticated routes all returned HTTP 200:

- `GET /api/health`
- `GET /api/lab/openstack`
- `GET /api/lab/cabinets`
- `GET /api/lab/roes`
- `GET /api/lab/topology`

The restricted OpenStack reader can list Nova servers, Neutron networks/ports/
subnets/security groups/rules, and Glance images. It cannot list hypervisors or
compute services; the dashboard does not use those calls as an ONLINE gate.
An admin-only read verified one hypervisor.

## OpenStack and Cabinet Topology

DevStack `stable/2026.2` has Keystone, Glance, Nova, Placement, Neutron and OVN
operational. Horizon is disabled; there is no public/provider network for this
lab. CirrOS 0.6.3 x86_64 is private in Glance with official SHA-256 verified.
Flavor `fs.nano` is 1 vCPU, 128 MiB RAM, 1 GiB disk.

Four Nova servers were ACTIVE at verification: `CENTRAL-01`, `CAB-A`, `CAB-B`,
`CAB-C`.

| Server | Neutron ports |
| --- | --- |
| CENTRAL-01 | FS-MGMT-A, FS-MGMT-B, FS-MGMT-C |
| CAB-A | FS-MGMT-A, FS-LINE-A |
| CAB-B | FS-MGMT-B, FS-LINE-B |
| CAB-C | FS-MGMT-C, FS-LINE-C |

No CENTRAL-01 port is on an FS-LINE network. The cabinets have no ports on
each other's network pairs. The pre-existing `private` network/router is
separate from these six private Geneve networks.

AAA contains 3 cabinets, 3 lines, 3 splitters and 1 ROE. CAB-A has `SPL-A-01`
and the one persisted ROE; CAB-B/C have their matching splitters and currently
no ROEs.

## Guest Isolation Evidence

The guest checks were genuinely executed at initial boot from CirrOS. All were
ICMP and TCP 22:

- CAB-A to CAB-B/CAB-C: BLOCKED; CAB-B/CAB-C to CAB-A: BLOCKED.
- CAB-B to CAB-C: BLOCKED; CAB-C to CAB-B: BLOCKED.
- Each CAB to CENTRAL-01 and CENTRAL-01 to each CAB: PASS.

The checks were not re-executed during the fingerprint change. The original
`verified_at` remains `2026-11-06T19:34:15Z`.

## Fingerprint Validation

The local mode-0600 `secrets/openstack-probes.json` stores the real guest
results, schema version 1 and this topology fingerprint:

`8dcb07d97da5cb496c671f4e024a6b4e391beb8bed3d46815d9f9e49105128c4`

The canonical JSON covers required Nova server IDs/statuses, Neutron network
and subnet IDs, port IDs and device/server and network mappings, fixed IPs,
security-group IDs and all rules attached to those groups. Collections are
sorted before serialization and SHA-256 hashing. The probe payload and original
timestamp were preserved when the fingerprint metadata was added.

OpenStack is ONLINE only if Keystone, Neutron, Nova, Glance pass, all four
required Nova servers are ACTIVE, all expected networks/ports are present, all
12 historical guest results match the required ICMP/TCP outcomes, the probe
schema/timestamp are valid, and the live topology fingerprint matches the
stored one. The response reports
`verification_mode=guest_probe_plus_unchanged_topology`,
`topology_unchanged=true`, original `last_guest_probe_at`, and informational
`guest_probe_age_seconds`. Age alone does not expire unchanged evidence.

Any relevant topology change, missing/invalid probe, failed probe or inactive
required Nova server yields DEGRADED/HTTP 503 and prevents a CAB from being
ACTIVE. A new real guest verification is required after a fingerprint change.
No timestamp or mtime is changed to simulate a recent probe.

## Authentication and Boundaries

- `CredentialProvider` separates authentication storage from handlers.
- `EnvCredentialProvider` reads local runtime settings and retains only token
  hashes in process memory; handlers receive `ClientPrincipal`, not raw token.
- `alex-web` has only the `dashboard` scope on the five GET routes above.
- Write methods, admin/internal, MCP write and secret routes are denied to ALEX.
- Key states are PENDING, ACTIVE, GRACE, REVOKED and EXPIRED. ACTIVE plus GRACE
  permits non-disruptive dual-key rotation. Invalid, unknown, revoked, expired
  keys and provider outages fail closed.
- Legacy API/admin/internal tokens remain separate and were not rotated here.
- Codex uses only registered MCP tools. Missing tools mean the action is not
  allowed; all writes pass Rust validation and audit. No shell, sudo, arbitrary
  SQL, Linux administration or direct OpenStack CLI is granted to Codex.

## ALEX and ZINGA

Lia reports that ALEX's direct read-only token calls are working. This RAGNO
handoff does not independently re-test ALEX's host or alter Apache/frontend.
ZINGA remains documented as a future independent broker/key manager only; no
ZINGA service, client or event bus was started.

## Verification and Recovery

- `cargo fmt --check`: PASS.
- `cargo test --locked`: 35 PASS.
- `cargo clippy --all-targets --locked -- -D warnings`: PASS.
- `git diff --check`: PASS.
- Live RAGNO API: five routes HTTP 200; OpenStack ONLINE; cabinets, ROEs and
  topology HTTP 200.

If the API becomes DEGRADED, inspect the JSON fields for missing service,
resource or fingerprint status. Do not restart OpenStack, modify Neutron, or
edit evidence timestamps. First confirm read-only server/port/network/subnet/
security-rule topology. If the fingerprint differs, stop reporting isolation
PASS until a new authorized guest execution method can genuinely rerun the
ICMP/TCP checks. No runner or timer is currently configured.
