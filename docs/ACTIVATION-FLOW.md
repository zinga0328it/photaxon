# FiberSpider Activation Flow

This document describes the activation workflow for a technician in the FiberSpider laboratory.

## 1. Technician identity

The workflow begins with a technician identifier submitted to the activation API.

## 2. Authorization

- The backend checks the technician ID against persisted PostgreSQL state.
- Only technicians with active status are authorized to proceed.

## 3. WR reference

- The activation request includes a work order reference (WR) as a stored field.
- WR is currently stored as a work-order reference.
- WR validation is NOT_IMPLEMENTED in the current web activation flow.

## 4. Telegram OTP

- The backend generates a one-time password (OTP).
- The OTP is hashed and stored securely in the database with a salt.
- The system sends the OTP via Telegram to the authorized technician.
- The technician must use the OTP to verify their identity.

## 5. ONT / GPON identification

- The technician provides ONT serial, ROE, cabinet, and splitter context.
- The laboratory models ONT detection and GPON path state as part of the activation record.
- Physical GPON hardware is not required for this branch; the identification step is part of the simulated workflow.

## 6. Path analysis

- The activation request captures cabinet, splitter, and ROE context.
- The system records logical resource topology for the requested field operation.

## 7. LLM proposal

- The LLM runtime and cabinet agent proposal mechanism are implemented as laboratory components.
- They are not yet directly invoked by the current `/api/activation/start` → `/api/activation/verify` workflow.
- The LLM component remains part of the simulation and proposal architecture rather than the active activation API path.

## 8. Deterministic backend validation

- The backend validates requests and topology decisions deterministically.
- Resource state transitions follow explicit rules and authorized technician context.
- The database holds authoritative state, while the LLM proposal remains advisory.

## 9. Persistent DB state

- Activation requests, OTP challenges, and resource states are stored in PostgreSQL.
- The `PostgresStore` implementation persists technician authorization and activation workflow data.
- Resource lease and deterministic state-transition support exists in the FiberSpider engine and PostgreSQL layer, but is not yet wired into the current web activation endpoint.

## 10. Provisioning workflow

- Once OTP verification completes, the workflow updates the activation status.
- The laboratory simulates provisioning steps in software.
- Resource leases and state transitions are recorded, but physical fiber provisioning remains simulated.

## 11. Final state

- The current endpoint returns `final_state = JOB_CLOSED` in the API response when the simulated workflow completes.
- In the database, `activation_requests.status` becomes `SIMULATION_COMPLETED`.
- The simulation result contains `final_state = JOB_CLOSED`.

> The workflow does not set `activation_requests.status` directly to `JOB_CLOSED`.

## Real vs simulated

- Real: technician authorization, Telegram OTP, OTP verification, PostgreSQL activation persistence, API workflow.
- Simulated: ONT detection, physical path confirmation, FTTH provisioning, GPON/OLT, JOB_CLOSED provisioning result.

The `pc-ragno` branch focuses on a laboratory proof-of-concept activation flow with persistent state and security controls, while physical FTTH execution remains part of the simulation model.