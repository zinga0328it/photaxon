# RAGNO Rust API

Status: active on RAGNO Yggdrasil; verified 2026-11-06.

## Routes

All routes below require the API bearer token. ALEX must inject it server-side;
the browser must never receive it.

- `GET /api/health`: authenticated API and PostgreSQL health.
- `GET /api/lab/openstack`: live Keystone, Glance, Nova, Neutron and OVN state.
- `GET /api/lab/cabinets`: AAA cabinet/splitter/ROE data joined with live Nova
  servers, Neutron networks/ports and fingerprint-bound guest evidence.
- `GET /api/lab/roes`: ROEs persisted in AAA.
- `GET /api/lab/topology`: AAA topology plus live cabinet status.

The OpenStack credential is read-only and loaded from the ignored local file
`secrets/openstack-api.env`. The probe snapshot is `secrets/openstack-probes.json`;
it is local, mode 0600, and holds the original guest verification timestamp,
results and a canonical topology SHA-256. Probe age is informational; the
evidence remains valid only while the live fingerprint matches. Nova/Neutron/
Glance resources are queried live on each request. Responses contain no
credential or OpenStack token.

## OpenStack and Cabinet Validation

A cabinet is returned as `ACTIVE` only when DB status and Nova server are
ACTIVE, its management and FTTH ports attach to its dedicated ACTIVE Neutron
networks, all CAB-to-CAB tests in the original guest probe are blocked, and
bidirectional ICMP/TCP connectivity to CENTRAL-01 passes. Those historical
results remain valid only while the current canonical SHA-256 topology
fingerprint matches. The guest timestamp is preserved and its age is
informational, not a TTL. A fingerprint change or invalid/missing evidence
returns OpenStack DEGRADED/503 and cabinet isolation STALE/FAIL; no cabinet is
ACTIVE until new real guest verification is available.

The restricted `reader` role can list servers, networks, ports and images. It
cannot list Nova hypervisors/services or read console output, so the OpenStack
response reports `hypervisor_count: null`; an admin CLI check verified one
hypervisor. The API does not claim that reader-only query as a pass.

The fingerprint covers required Nova server IDs/statuses, Neutron network and
subnet IDs, port IDs and mappings, fixed IPs, security-group IDs and their
rules. Arrays are sorted before JSON serialization and SHA-256 hashing.

## Historical Initial Slice

The first modular API smoke-test report described only the health/AAA routes
and a temporary test server. That was a pre-DevStack milestone and is superseded
by the active Rust user unit and the live lab routes documented above.