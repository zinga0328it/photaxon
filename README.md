# Photaxon — FiberSpider
Distributed FTTH Research Laboratory

This public branch documents the FiberSpider laboratory proof of concept for the PC RAGNO node.

FiberSpider is a distributed FTTH laboratory architecture that combines:

- stable logical identity and persistent state
- dynamic distributed cabinet topology
- technician authorization and OTP-backed activation
- simulated FTTH path analysis and deployment workflows
- OpenStack laboratory network integration

## Branch role: pc-ragno

`pc-ragno` contains the RAGNO laboratory backend and the FiberSpider activation API.

- FastAPI activation service for technician workflows
- Telegram OTP-based technician verification
- PostgreSQL persistence for activation requests, OTP challenges, and resource state
- Cabinet agent simulation for CAB-A, CAB-B, CAB-C
- LLM-assisted topology proposal and deterministic backend validation
- Proof-of-concept FTTH provisioning simulation, not physical GPON deployment

## Documentation

See the following documents for details:

- `docs/ARCHITECTURE.md`
- `docs/BRANCHES.md`
- `docs/LAB-STATUS.md`
- `docs/ACTIVATION-FLOW.md`
- `docs/SECURITY-MODEL.md`
- `docs/ROADMAP.md`
- `docs/RESEARCH-AND-FUNDING.md`

## Research and funding

This repository is a public research and demonstration effort. All rights reserved. Commercial use, deployment, redistribution, or integration requires written authorization and a separate commercial agreement with the project owner.

## Safety note

This branch is a curated public snapshot. It does not contain operational credentials, sensitive infrastructure configuration, or private secrets.