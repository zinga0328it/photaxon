# RAGNO OpenStack Precheck

Date: 2026-11-06
Scope: read-only inventory before DevStack installation.

## Host

- Hostname: `ragno`.
- OS: Ubuntu 24.04.4 LTS; kernel `6.8.0-147-generic`.
- CPU: Intel Core i3-7100U, 2 physical cores / 4 threads.
- Hardware virtualization: Intel VT-x present; `/dev/kvm` exists, but the
  current login user cannot access it directly.
- RAM: 7.7 GiB total, about 5.0 GiB available during the check, 4 GiB swap.
- Storage: 512 GB SATA SSD; ext4 root filesystem, 442 GiB total, 406 GiB free.
- SMART health: `PASSED`.
- AppArmor: active.
- UPS telemetry: unavailable; blackout protection is not verified.

## Network and Existing Services

- Existing interfaces: `lo`, `enp2s0`, `wlp1s0`, `tun0`; 3 IPv4 routes and 5
  IPv6 routes. Addresses and route contents are intentionally omitted.
- SSH: active.
- Yggdrasil: active; `sudo -n yggdrasilctl getSelf` succeeded. Public identity
  was not recorded in this repository.
- nftables: active; read-only ruleset access succeeded. Rules were not changed.
- Apache2: active and owns ports 8080 and 8081. Port 80 and the standard
  OpenStack API ports checked were not listening during the precheck. DevStack
  may still alter shared Apache configuration, so Horizon startup requires
  checking for service/config conflicts.
- No default route, firewall, SSH, Apache, or Yggdrasil changes were made.

## OpenStack State

- No OpenStack CLI, DevStack checkout, OVN, Neutron, or Open vSwitch installation
  was present before this precheck. QEMU system packages had been installed
  earlier; no VM was created.
- The official DevStack Git remote contains `stable/2026.2` at commit
  `e66e72e0cdb4e652df430e6afb8cfa24fb8e178e`.
- Official general DevStack documentation recommends Ubuntu 24.04 as the most
  tested current LTS and warns that DevStack makes substantial host changes and
  should run on a dedicated server or VM. The branch-specific documentation URL
  was unavailable during this check, so support details for that exact branch
  remain unverified.
- The host has 2 cores and 7.7 GiB RAM. This is a constrained single-node
  development host and will have limited headroom while MySQL, RabbitMQ,
  Keystone, Glance, Nova, Neutron/OVN, Placement, Horizon, Apache, and
  FiberSpider run together.

## Precheck Result

Ubuntu 24.04 and the requested stable branch are present. Required existing
services are active, Yggdrasil's control check and SSD SMART passed, and the
current firewall was readable. UPS protection and exact branch documentation
remain unverified. No installation or configuration changes were made during
this precheck.