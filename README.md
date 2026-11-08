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

- Rust API and private ChatGPT MCP control plane (112 live tools)
- Telegram private ACTIVE technician identity; OTP reserved for the future WR web panel
- PostgreSQL persistence for activation requests, OTP challenges, and resource state
- Cabinet agent simulation for CAB-A, CAB-B, CAB-C
- Laboratory LLM runtime and topology proposal support, with deterministic backend validation
- Proof-of-concept FTTH provisioning simulation, not physical GPON deployment

Current verified implementation: private OpenAI Secure MCP Tunnel, named Rust
services and AAA transactions/audit; physical asset models and legacy provenance,
WR/photo catalogs and manual ONE acceptance with mandatory test code. The digital
twin and external delivery automation remain partial. No local database is used.

## Documentation

See the following documents for details:

- `docs/ARCHITECTURE.md`
- `docs/ARCHITETTURA.md` (current architecture and complete tool groups)
- `docs/OBIETTIVI.md` (verified live state and pending work)
- `docs/CHATGPT-MCP.md` (private tunnel runbook)
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