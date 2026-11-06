# Neutron Lab Topology

Status: deployed and guest-tested on RAGNO, 2026-11-06.

## Networks

| Cabinet | Management network | FTTH network |
| --- | --- | --- |
| CAB-A | FS-MGMT-A | FS-LINE-A |
| CAB-B | FS-MGMT-B | FS-LINE-B |
| CAB-C | FS-MGMT-C | FS-LINE-C |

All six are tenant Geneve networks with private IPv4 DHCP subnets and no
gateway. There is no router, provider network, or public network attached to
these segments. The pre-existing `private` network/router is separate.

CENTRAL-01 has one port on each `FS-MGMT-*` network and no FTTH port. Each
cabinet has exactly two ports: its own `FS-MGMT-*` and `FS-LINE-*` networks.
No cabinet port exists on another cabinet's networks.

## Security Policy and Tests

- CAB security groups permit ICMP and TCP 22 ingress only from that CAB's
	CENTRAL-01 management address; default egress is unchanged.
- CENTRAL-01 permits ICMP and TCP 22 from the three management subnets.
- All six directional CAB-to-other-CAB tests returned ICMP BLOCKED and TCP
	BLOCKED from inside the CirrOS guests.
- All six directional CENTRAL-01-to-CAB and CAB-to-CENTRAL tests returned
	ICMP PASS and TCP PASS from inside the guests.
- No host nftables rule was used to simulate isolation.

The probe results are retained on RAGNO in an ignored mode-0600 file and are
considered current for one hour. Rust API cabinet status is INACTIVE when any
DB/Nova/network/probe condition is missing or stale.
# Neutron Lab Topology

Status: proposed; no Neutron resources exist yet.

## Placement

OpenStack and Neutron ML2/OVN run inside the isolated lab VM on RAGNO. ALEX
remains the frontend/Apache reverse-proxy node. The application path is:

```text
Browser -> ALEX/Apache -> RAGNO/FiberSpider -> AAA/PostgreSQL
```

## Logical Networks

Create the `photaxon-lab` project with separate private tenant networks for
CAB-A, CAB-B, and CAB-C. Add a management network only if required by the lab
agents. Select IPv4 CIDRs only after checking all existing private subnets;
no CIDRs are assigned because ALEX/AAA network ranges have not been inventoried.

Use ML2/OVN switching inside the VM. Do not attach provider/public networks,
physical NICs, the host management interface, or Yggdrasil. Any router must
remain internal, without an external gateway. Use default-deny inbound security
groups and allow only explicitly required lab traffic.

Do not connect simulated cabinet networks to GPON, operator infrastructure, or
real devices.

## Not Applied

No project, networks, subnets, router, security groups, OVN chassis, or virtual
machines have been created. Resource creation waits until an isolated VM can
be prepared without starving or interrupting host services.
