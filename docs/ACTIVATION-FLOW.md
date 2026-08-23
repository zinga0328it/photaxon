# FiberSpider Activation Flow

This document describes the activation workflow for a technician in the FiberSpider laboratory.

## 1. Technician identity

The workflow begins with a technician identifier submitted to the activation API.

## 2. Authorization

- The backend checks the technician ID against persisted PostgreSQL state.
- Only technicians with active status are authorized to proceed.

## 3. WR reference

- The activation request includes a work order reference (WR).
- The backend stores the WR and associated field metadata in the activation request record.

## 4. Telegram OTP

- The backend generates a one-time password (OTP).
- The OTP is hashed and stored securely in the database with a salt.
- The system sends the OTP via Telegram to the authorized technician.
- The technician must use the OTP to verify their identity.

## 5. ONT / GPON identification

- The technician provides ONT serial and cabinet/ splitter context.
- The laboratory models ONT detection and GPON path state as part of the activation record.
- Physical GPON hardware is not required for this branch; the identification step is part of the simulated workflow.

## 6. Path analysis

- The activation request captures cabinet, splitter, line, and ROE context.
- The system records logical resource topology for the requested field operation.

## 7. LLM proposal

- The laboratory includes an LLM runtime for agent proposals and field interpretation.
- The LLM model helps produce a topology proposal based on the technician’s input and the cabinet context.
- This is a controlled laboratory simulation and not a production network decision engine.

## 8. Deterministic backend validation

- The backend validates requests and topology decisions deterministically.
- Resource state transitions follow explicit rules and authorized technician context.
- The database holds authoritative state, while the LLM proposal remains advisory.

## 9. Persistent DB state

- Activation requests, OTP challenges, and resource states are stored in PostgreSQL.
- The `PostgresStore` implementation persists technician authorization, activation workflow data, and resource lease state.

## 10. Provisioning workflow

- Once OTP verification completes, the workflow updates the activation status.
- The laboratory simulates provisioning steps in software.
- Resource leases and state transitions are recorded, but physical fiber provisioning remains simulated.

## 11. JOB_CLOSED

- The endpoint returns a final state and simulation result when the workflow completes.
- The process ends with the activation marked as closed in the database.

## Real vs simulated

- Real: technician authorization, OTP challenge, database persistence, API activation flow, backend validation.
- Simulated: physical ONT provisioning, GPON transport, actual fiber path switching, hardware integration.

The `pc-ragno` branch focuses on a laboratory proof-of-concept activation flow with persistent state and security controls, while physical FTTH execution remains part of the simulation model.