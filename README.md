# Photaxon — FiberSpider
Distributed FTTH Research Laboratory

FiberSpider is an experimental distributed FTTH architecture developed under the Photaxon research project.

The project investigates intelligent distributed cabinet nodes, LLM-assisted network reasoning, deterministic network control, persistent topology state, OpenStack/Neutron laboratory simulation, secure technician workflows and resilient FTTH automation.

## The problem

Traditional FTTH workflows often depend on pre-assigned physical topology and centralized operational information.

FiberSpider researches a different model based on:

- stable logical identity
- dynamic physical path
- distributed intelligent nodes
- authoritative persistent state

## Architecture

Browser
   |
   v
ALEX
Public frontend / Apache reverse proxy
   |
   v
RAGNO
FiberSpider backend / agents / OpenStack laboratory
   |
   v
AAA
PostgreSQL persistent network state

CAB-A <--> CAB-B <--> CAB-C

Distributed cabinet laboratory nodes.

## Public branches

- [pc-ragno](https://github.com/zinga0328it/photaxon/tree/pc-ragno): Backend, FastAPI activation laboratory, cabinet agents, LLM runtime, OpenStack integration and simulation engine.
- [pc-aaa](https://github.com/zinga0328it/photaxon/tree/pc-aaa): PostgreSQL schema and persistent FiberSpider state model.
- [pc-alex](https://github.com/zinga0328it/photaxon/tree/pc-alex): Public Photaxon frontend and sanitized Apache reverse-proxy example.

The branches intentionally remain separate because each branch represents a different physical laboratory node.

## Current status

### WORKING SOFTWARE PROOF OF CONCEPT

- FastAPI activation workflow
- technician authorization against PostgreSQL
- Telegram OTP delivery and verification
- PostgreSQL activation persistence
- cabinet agent laboratory components
- OpenStack/Neutron laboratory environment
- public Photaxon frontend
- distributed three-node architecture

IMPORTANT:

- WR is currently stored as a reference.
- WR validation is NOT_IMPLEMENTED.
- LLM runtime exists but is not directly invoked by the current web activation endpoint.
- Resource lease/state-transition support exists in the engine/database layer but is not yet wired into the current web activation endpoint.

### SIMULATED

- ONT detection
- physical FTTH path confirmation
- provisioning execution
- GPON/OLT behavior
- physical fiber switching
- JOB_CLOSED provisioning result

### PLANNED HARDWARE RESEARCH

- physical ONT integration
- mini-OLT / GPON laboratory
- smart cabinet sensors
- distributed active cabinet prototype
- real field pilot

## Research, partnerships and funding

Photaxon is seeking collaboration for the next research phase.

Interests:

- research funding
- telecommunications operators
- universities and research institutes
- networking / optical hardware partners
- smart cabinet prototyping
- FTTH pilot locations
- distributed network research
- commercial technology partnerships

The software proof of concept serves as a basis for designing and testing future FiberSpider hardware.

## Licensing

Copyright © Photaxon / FiberSpider.
All rights reserved.

This repository is publicly visible for research, demonstration and partnership evaluation.

No open-source license is granted.

Commercial use, deployment, redistribution, integration, derivative commercial implementation or production adoption requires prior written authorization and a separate commercial licensing agreement with the project owner.
