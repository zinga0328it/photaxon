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
  return HTTP 200. OpenStack reports ONLINE and CAB-A/B/C report ACTIVE only
  with DB, Nova, Neutron ports, fresh probe evidence, isolation and central
  connectivity all verified.
- API application credential is Keystone `reader`, restricted, expires
  2027-02-04. Credential and probe evidence are local files under ignored
  `secrets/`, mode 0600; no values are stored in Git.

## ALEX Gate

- ALEX was not accessed or changed. The live homepage still renders static
  CAB-A/B/C and its JavaScript expects an obsolete API schema and `/lab/demo`.
- Browser fetches to the public API prefix time out; Cloudflare reports HTTP
  524. Direct ALEX-to-RAGNO Yggdrasil reachability is not verified.
- See the local-only `secrets/ALEX-HANDOFF.md` for the exact ProxyPass,
  server-side token, dynamic cabinet rendering, and acceptance checks.

No commit or push was made. Do not mark the overall Photaxon site rollout done
until ALEX applies the handoff and all five public URLs plus the dynamic page
are verified.
# OpenStack Installation and Test Report

Date: 2026-11-06
Host: RAGNO, Ubuntu 24.04.4 LTS
Result: OpenStack not installed; no VM created.

## Inventory

- CPU: Intel Core i3-7100U, 2 physical cores / 4 threads; VT-x and `/dev/kvm`
  available. Target VM requires 4 vCPU plus host headroom.
- RAM: 7.7 GiB total, about 5.2 GiB available during inspection; target VM
  requires 16 GiB plus host headroom.
- Disk: 512 GB SATA SSD, ext4 root, 407 GiB free; 100 GiB target disk fits.
- AppArmor is active; the host is not itself inside a detected VM.
- SSH, Yggdrasil, nftables, and Apache2 are active on RAGNO. ALEX was not
  inspected remotely.
- No OpenStack, DevStack, Open vSwitch, OVN, Neutron, Nova, or libvirt
  components were found. QEMU and `build-essential` were installed for isolated
  VM/Rust preparation; no guest image or VM has been created.
- Yggdrasil identity and SSD SMART status could not be verified because the
  required non-interactive sudo checks were unavailable. UPS protection is
  unverified because no telemetry tool is installed.

## Verified Checks

- AAA PostgreSQL: read-only `SELECT 1`, `PostgresStore.healthcheck()`, and
  topology reads passed. Counts: 3 cabinets, 3 lines, 3 splitters, 3 agents.
- Offline Python suite: 50 tests, 23 passed and 27 DB integration tests skipped
  by their safety gate. Standalone offline agent checks passed.
- The modular Rust release API passed 3 unit tests, formatting and Clippy; 7
  authenticated HTTP checks passed over the local Yggdrasil address, including
  DB-backed cabinet reads. Its listener was stopped afterward. OpenStack status
  remains `OFFLINE`.
- No database rows, SSH, nftables, Apache, Yggdrasil, host networking, or
  system service configuration were modified.

## Decision

The host does not meet the VM CPU/RAM targets. The user requested an under-sized
experiment; QEMU packages are present, but cloud-image tooling and a guest VM
are not. Further privileged setup is blocked because `sudo -n` is unavailable
in the agent terminal. Do not switch to a direct-host install: it would risk
the active Apache, SSH, Yggdrasil, and nftables services.

No OpenStack versions/services, ML2/OVN, Horizon, cloud credentials, SDK
integration, or VM services are installed or verified. Rustup and
`build-essential` were added; no persistent service, firewall, or network
configuration was changed. No Git push was made.
