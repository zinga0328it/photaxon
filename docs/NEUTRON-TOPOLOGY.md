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

The original real guest results are retained on RAGNO in an ignored mode-0600
file with their original verification timestamp. A canonical SHA-256 covers
the required Nova server IDs/statuses, network/subnet IDs, port IDs and mappings,
security-group IDs and all associated rules. PASS remains valid while that
fingerprint is unchanged; probe age alone does not expire it. Any mismatch or
invalid/missing probe forces DEGRADED and prevents cabinets from being ACTIVE.

Fingerprint verified on 2026-11-06:
`8dcb07d97da5cb496c671f4e024a6b4e391beb8bed3d46815d9f9e49105128c4`.
The recorded guest verification time remains `2026-11-06T19:34:15Z`; this
fingerprint association does not claim that guest commands were re-run then.

## Refresh Limitation

Guest user-data executes only on boot; the current lab has no usable SSH keypair
or tenant route, no serial console, and an inactive loopback-only VNC proxy.
The existing guest results are bound to the verified topology fingerprint;
they remain valid until a relevant server, port, network, subnet or security
policy changes. Do not reset mtime or rewrite probe results. On mismatch, require
a new real guest verification before reporting PASS.
