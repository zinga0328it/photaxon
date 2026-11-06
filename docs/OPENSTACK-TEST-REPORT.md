# OpenStack Installation and Test Report

Date: 2026-11-06
Host: RAGNO, Ubuntu 24.04.4 LTS
Result: RAGNO lab ready; ALEX handoff pending.

## Verified

- DevStack `stable/2026.2`; Keystone token, Glance image list, Nova server
  list, Neutron network list and OVN Geneve backend pass.
- Nova has one registered hypervisor and four active CirrOS servers:
  `CENTRAL-01`, `CAB-A`, `CAB-B`, `CAB-C`.
- CirrOS 0.6.3 x86_64 is private in Glance; official SHA-256 verified.
- `fs.nano`: 1 vCPU, 128 MiB RAM, 1 GiB disk.
- Six tenant Geneve networks are active, each with a DHCP subnet and no
  gateway. No public/provider network or router was added for this lab.
- CENTRAL-01 has only the three management ports. Each CAB has only its own
  management and FTTH ports.
- Guest ICMP/TCP 22 tests: all six CENTRAL-to-CAB/CAB-to-CENTRAL paths pass;
  all six CAB-to-other-CAB paths are blocked in both protocols.
- AAA: 3 DB cabinets and 1 ROE. CAB-A has `SPL-A-01` and one ROE; CAB-B/C
  have their own splitters and currently no ROEs.
- Rust API is active on RAGNO Yggdrasil. All five authenticated API routes
  return HTTP 200. OpenStack reports ONLINE and CAB-A/B/C report ACTIVE with
  DB, Nova, Neutron ports, the historical guest probe PASS, and an unchanged
  topology fingerprint.
- API application credential is Keystone `reader`, restricted, expires
  2027-02-04. Credential and probe evidence are local files under ignored
  `secrets/`, mode 0600; no values are stored in Git.

## ALEX Gate

- Lia reports ALEX's read-only token and direct calls to health, cabinets,
  ROEs, and topology as working. These remote results were not independently
  re-run during this RAGNO-only documentation pass.
- This task did not modify ALEX, Apache, or the public frontend. The public
  ProxyPass/frontend rollout is separate and is not claimed as verified here.
- See the local-only `secrets/ALEX-HANDOFF.md` for prior deployment notes; do
  not copy local secret material into Git.

This document now describes the RAGNO control plane. It does not assert the
status of ALEX's public vhost or browser rendering.

## Probe Validity

- The guest probes were genuinely executed at initial boot and passed for all
  12 directional ICMP/TCP expectations. Their original `verified_at` remains
  `2026-11-06T19:34:15Z`; it is not rewritten or treated as a fresh run.
- The probe file now stores schema version 1 and SHA-256 fingerprint
  `8dcb07d97da5cb496c671f4e024a6b4e391beb8bed3d46815d9f9e49105128c4`, bound
  to the live server/port/network/subnet/security-group/rule topology.
- The evidence remains valid while the canonical fingerprint is unchanged.
  A mismatch, invalid/missing evidence, changed required Nova state, or
  incomplete probe set returns DEGRADED/503 and marks isolation STALE/FAIL.
- `guest_probe_age_seconds` is informational only; there is no age-based TTL.
  No timestamp was fabricated, no VM was restarted, and no topology changed.
- At final verification the original guest timestamp remained
  `2026-11-06T19:34:15Z`, age 7207 seconds; `/api/lab/openstack` still returned
  HTTP 200 because the live topology fingerprint matched.
- A guest command runner/timer is not needed for this fingerprint model. If the
  topology changes, a new real guest verification is still required; no safe
  guest exec channel is currently available to perform it automatically.
