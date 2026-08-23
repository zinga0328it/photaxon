# FiberSpider Security Model

This document describes the security model for the FiberSpider public laboratory branch.

## Core principle

- The LLM proposes.
- The backend validates.
- The database persists authoritative state.

This separation ensures that advisory intelligence is distinct from the authoritative implementation layer.

## OTP

- One-time passwords are generated in memory and stored as secure hashes.
- OTP exchange uses Telegram as an external delivery channel.
- OTP state is persisted in the database with salt and expiration.

## Resource leases

- The laboratory models resource leasing for fiber path elements.
- Resource leases provide temporary ownership and avoid conflicting actions on the same path or port.
- Lease state is persisted in PostgreSQL.

## Audit and events

- The system records activation events, agent proposals, and resource state transitions.
- Audit trails are maintained through deterministic backend state updates.
- The objective is traceability for technician actions and simulation decisions.

## Network security components

- nftables is part of the architecture for network traffic filtering and isolation in the laboratory environment.
- Falco is referenced as a host-level security monitoring component for runtime anomaly detection.
- These components are external to the repository and enforce layered security around the lab.

## Secrets handling

- Secret values are not stored in this public repository.
- Environment variables are used for credentials and tokens.
- Telegram bot tokens, database passwords, and operational credentials remain external to the code base.
- The public branch documents the required environment variables without exposing real values.

## Public-safe description

This document focuses on architecture and security design only. It does not disclose private configuration, operational IP addresses, or any real secrets.