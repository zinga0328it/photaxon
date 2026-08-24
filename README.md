# Photaxon — FiberSpider
Distributed FTTH Research Laboratory

This public branch documents the FiberSpider laboratory proof of concept for the PC RAGNO node.

FiberSpider is a distributed FTTH laboratory architecture that combines:

- stable logical identity and persistent state
- dynamic distributed cabinet topology
- technician authorization and OTP-backed activation
- simulated FTTH path analysis and deployment workflows
- OpenStack laboratory network integration

## Branch role: rd-reproducible-demo

`rd-reproducible-demo` contains the RAGNO laboratory backend and the FiberSpider activation API.

- FastAPI activation service for technician workflows
- Telegram OTP-based technician verification
- PostgreSQL persistence for activation requests, OTP challenges, and resource state
- Cabinet agent simulation for CAB-A, CAB-B, CAB-C
- Laboratory LLM runtime and topology proposal support, with deterministic backend validation
- Proof-of-concept FTTH provisioning simulation, not physical GPON deployment

## Reproduce the demo

```bash
git clone --branch rd-reproducible-demo https://github.com/zinga0328it/photaxon.git
cd photaxon
cp .env.example .env
docker compose up --build -d
curl http://127.0.0.1:8000/api/health
docker compose run --rm backend python -m demo.run_demo
```

See [the repeatable procedure](docs/REPRODUCIBLE-DEMO.md), the
[architecture diagrams](docs/ARCHITECTURE-DIAGRAM.md), and the
[Smart Cabinet telemetry contract](docs/SMART-CABINET-TELEMETRY.md).

## Verification status

| Area | Status |
|---|---|
| FastAPI, deterministic resource engine and storage | working software |
| PostgreSQL demo persistence | working software |
| Cabinet event input and FTTH physical path | simulated |
| LLM decision in the repeatable demo | simulated, advisory only |
| Telegram OTP and OpenStack | optional laboratory integrations |
| Physical ONT/OLT, sensors, LEDs and provisioning | future / not implemented |

The LLM agent proposes. Deterministic backend policy validates. Only authorized
code reserves resources and updates PostgreSQL.

## Documentation

See the following documents for details:

- `docs/ARCHITECTURE.md`
- `docs/BRANCHES.md`
- `docs/LAB-STATUS.md`
- `docs/ACTIVATION-FLOW.md`
- `docs/SECURITY-MODEL.md`
- `docs/ROADMAP.md`
- `docs/RESEARCH-AND-FUNDING.md`
- `docs/ARCHITECTURE-DIAGRAM.md`
- `docs/SMART-CABINET-TELEMETRY.md`
- `docs/REPRODUCIBLE-DEMO.md`

## Research and funding

This repository is a public research and demonstration effort. All rights reserved. Commercial use, deployment, redistribution, or integration requires written authorization and a separate commercial agreement with the project owner.

## Safety note

This branch is a curated public snapshot. It does not contain operational credentials, sensitive infrastructure configuration, or private secrets.

## Mobile workflow verification

This reproducibility branch was prepared and committed from Termux on Android, demonstrating a Git-based development workflow from a mobile Linux environment.
