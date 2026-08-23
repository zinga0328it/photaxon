# FiberSpider Lab Status

This document summarizes the current implementation status of the FiberSpider laboratory in the `pc-ragno` public branch.

## IMPLEMENTED

- FastAPI activation backend for technician activation requests.
- OTP generation and challenge flow with Telegram-backed verification support.
- PostgreSQL persistence for activation requests and OTP challenge state.
- Cabinet agent logic for field events and technician arrival.
- OpenStack laboratory integration as a simulated networking/runtime environment.
- Resource state persistence and deterministic state transitions.

## WORKING

- Technician authorization against persisted PostgreSQL state.
- Activation request lifecycle from OTP request to verification.
- Simulated path analysis and activation completion.
- Cabinet-level agent proposals via LLM runtime simulation.
- Persistent storage of activation requests, OTP challenges, and resource leases.

## SIMULATED

- FTTH provisioning and physical path execution.
- Cabinet field operations such as ONT discovery and splitter path assignment.
- Laboratory path topology decisions and provisioning actions.
- Physical GPON/OLT behavior is modeled in software rather than executed on real hardware.

## PLANNED

- physical ONT integration
- mini-OLT / GPON integration
- smart cabinet sensors for real field telemetry
- distributed field cabinet prototype deployment
- pilot deployment with research partners

> Note: no physical GPON provisioning is currently implemented in this branch. The laboratory remains a proof-of-concept simulation environment.