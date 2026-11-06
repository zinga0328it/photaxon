# OpenStack on RAGNO

Status: single-node DevStack lab operational; constrained host. Verified
2026-11-06. See `OPENSTACK-TEST-REPORT.md` for results.

## Deployment

- DevStack branch: `stable/2026.2`.
- Nova/KVM, Glance, Keystone, Neutron ML2/OVN and Placement are operational.
- Horizon, public/provider networks and external gateways are not used.
- Glance has private CirrOS 0.6.3 x86_64; its official SHA-256 was checked.
- Nova flavor `fs.nano`: 1 vCPU, 128 MiB RAM, 1 GiB disk.
- Nova servers: `CENTRAL-01`, `CAB-A`, `CAB-B`, `CAB-C`, all ACTIVE.

## Runtime Boundaries

- Rust API remains bound only to RAGNO's Yggdrasil IPv6 listener on port
  18000. It reads OpenStack via a restricted Keystone application credential.
- The API does not create, delete, or mutate OpenStack resources.
- Guest-probe evidence is stored locally with mode 0600 and bound to the
  canonical OpenStack topology fingerprint. Probe age is reported but does not
  expire an unchanged topology; mismatch/missing/invalid evidence fails closed.
- The host has 2 physical CPU cores and 7.7 GiB RAM. This is a constrained
  lab, not a production-capacity OpenStack deployment.
- Apache, Yggdrasil, nftables and SSH were not altered in this phase.

## ALEX

ALEX/Apache and the public frontend are outside this RAGNO deployment. Lia
reports direct read-only API access from ALEX as working; this file does not
claim an Apache/frontend deployment. A local-only handoff remains in
`secrets/ALEX-HANDOFF.md`.

## Historical Precheck (Before DevStack Installation)

The following inventory records the pre-install state and is retained for
history only. It is superseded by the operational status at the top of this
document. Resource constraints identified then remain relevant.

### Intended Method (Historical)

Run a supported single-node OpenStack release with Neutron ML2/OVN inside a
dedicated VM on RAGNO. Never install DevStack/OpenStack directly on the
operational host. Keep the VM on private user-mode/NAT networking; do not pass
through physical or Yggdrasil interfaces or expose a public network.

The requested supported VM allocation is at least 4 vCPU, 16 GiB RAM, and
100 GiB SSD, with additional host resources for FiberSpider. The user has
requested attempting an under-sized lab despite these targets. Any such
experiment is unsupported and may fail; it must remain isolated from host
services and must not be reported as a successful deployment unless verified.

### Host Inventory (Historical)

- OS: Ubuntu 24.04.4 LTS; kernel `6.8.0-147-generic`.
- CPU: Intel Core i3-7100U, 2 physical cores / 4 threads; VT-x present.
- Virtualization: no outer hypervisor detected; `/dev/kvm` present.
- Memory: 7.7 GiB total, about 5.2 GiB available at inspection, 4 GiB swap.
- Storage: 512 GB SATA SSD; root is ext4, 442 GiB total, 407 GiB free.
- AppArmor is active.
- SSH, Yggdrasil, nftables, and Apache2 are active on the inspected RAGNO
  host. The separate ALEX host was not inspected.
- At the pre-install inventory, OpenStack, DevStack, Open vSwitch, OVN,
  Neutron, and Nova were absent; this snapshot is historical only.
  or active. QEMU system packages are installed, but libvirt, `cloud-localds`,
  and a guest VM are absent.
- RAGNO-to-AAA PostgreSQL health and read-only topology checks succeeded;
  counts were 3 cabinets, 3 lines, 3 splitters, and 3 agents. No rows changed.
- Yggdrasil identity retrieval and SMART health were not verified because
  non-interactive sudo was unavailable. UPS/blackout protection is not
  verified; no UPS telemetry client is installed.

The host is below the requested VM CPU/RAM targets. SSD capacity is sufficient,
but it does not compensate for those deficits. Exact host addresses and
database credentials are intentionally omitted from this public repository.

### Decision at Precheck Time (Historical)

Use a dedicated host that can allocate 4 vCPU, 16 GiB RAM, and 100 GiB SSD to
the OpenStack VM while retaining FiberSpider host headroom. Until then, keep
RAGNO on the existing database connection and FiberSpider simulation mode.
