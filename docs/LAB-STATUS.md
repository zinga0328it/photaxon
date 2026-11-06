# FiberSpider Lab Status

Status verified on RAGNO, 2026-11-06. Physical FTTH provisioning remains a
simulation/prototype; the OpenStack control-plane lab is real and isolated.

## Implemented and Verified

- Rust Axum API is the primary RAGNO runtime; Python/FastAPI activation code is
	legacy/demo tooling.
- Technician authentication/OTP and AAA PostgreSQL persistence remain present.
- DevStack `stable/2026.2` runs Keystone, Glance, Nova, Placement,
	Neutron ML2/OVN on the constrained RAGNO host.
- Nova runs `CENTRAL-01`, `CAB-A`, `CAB-B`, and `CAB-C` as CirrOS lab servers.
- Three separate cabinet management/FTTH network pairs use private Geneve
	tenant networks; central has management-only ports.
- Rust reads OpenStack and AAA live. Cabinet ACTIVE status requires DB/Nova,
	dedicated networks/ports, a passing guest probe record and a matching live
	topology fingerprint.
- Scoped client auth gives `alex-web` only the `dashboard` GET scope; legacy
	API/admin/internal credentials remain distinct.

See `RAGNO-FINAL-HANDOFF.md` for exact endpoints, topology, test results and
the fingerprint-bound guest verification model.

## Simulated or Planned

- Physical GPON/OLT provisioning and real ONT operations are not performed.
- Cabinet hardware, optical sensors, resilient power and field telemetry are
	research/prototype work.
- ALEX public Apache/frontend deployment is owned separately by ALEX; this
	RAGNO status does not claim that public-site changes were applied.
- ZINGA remains a future independent broker/key-manager server; no broker or
	event bus is implemented here.