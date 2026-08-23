# AAA Node for FiberSpider

This document describes the role of the AAA node in the FiberSpider architecture. AAA is a persistent PostgreSQL node used to store FiberSpider state and topology data for a fiber network simulation.

## Role of AAA

AAA is responsible for:

- persistent PostgreSQL storage for FiberSpider
- maintaining network state and topology
- storing cabinet and FTTH line metadata
- tracking splitter configurations and ONT endpoints
- recording technician activity and assignments
- managing activation requests and OTP challenge state
- preserving transaction records and anomalies
- tracking agent state and audit/state persistence

## Data model focus

AAA stores data related to the physical and operational state of a fiber network, including:

- cabinets and distribution points
- FTTH lines and connections
- splitters and logical fiber segments
- ONTs and endpoint equipment
- technician assignments and work orders
- activation and provisioning requests
- one-time-password (OTP) challenge events
- transaction history and anomaly detection
- agent status and state persistence

## Public-safe description

This node is a local database backend for FiberSpider. It is designed to preserve state across restarts and support the application workflow for tracking network topology, provisioning, activations, and monitoring. No sensitive production credentials, IP addresses, operational user names, or secret data are included in this public documentation.
