# FiberSpider Architecture

FiberSpider is a distributed FTTH research laboratory developed by Photaxon. This branch documents the public proof-of-concept architecture for the PC RAGNO node and its integration with the broader FiberSpider laboratory.

## Logical node roles

- Browser: public user or technician browser interface.
- ALEX: public frontend and reverse proxy node.
- RAGNO: FiberSpider laboratory backend node with agent and OpenStack integration.
- AAA: persistent PostgreSQL state node.

### Browser → ALEX → RAGNO → AAA

- Browser connects to ALEX over HTTPS.
- ALEX serves the public demo frontend and forwards API traffic to the FiberSpider backend.
- RAGNO runs the FiberSpider agent backend, activation API, and laboratory simulation.
- AAA stores the authoritative persistent state for technician authorization, activation workflows, resource leases, and topology data.

## RAGNO

RAGNO is the active laboratory node in this branch.

- Runs the FiberSpider backend and FastAPI activation service.
- Integrates with OpenStack/Neutron for laboratory networking simulation.
- Hosts the agent runtime for cabinet processing.
- Orchestrates activation, OTP verification, and simulated provisioning workflows.

## AAA

AAA is the persistent database node.

- Provides PostgreSQL-backed persistence for FiberSpider state.
- Stores technicians, activation requests, OTP challenge history, resource states, and lease records.
- Preserves logical state across lab restarts.

## ALEX

ALEX is the public-facing frontend node.

- Serves the demo web UI and public dashboard.
- Acts as the Apache reverse proxy for the FiberSpider backend.
- Separates public presentation from laboratory logic.

## Cabinets as distributed laboratory nodes

The FiberSpider laboratory models distributed FTTH cabinets:

- CAB-A: primary demonstration cabinet and field node.
- CAB-B: secondary cabinet node.
- CAB-C: tertiary cabinet node.

Each cabinet is treated as a logical distributed node with local agent behavior, topology context, and simulated resource state.

## FiberSpider principles

FiberSpider is built around two complementary principles:

- Stable identity and logical state:
  - technicians, cabinets, resources, and activations have stable identifiers.
  - authoritative state is persisted in the database.

- Dynamic physical path and resources:
  - the physical FTTH path is modeled as a dynamic topology.
  - resources such as splitter ports, ONTs, and cabinet connections can be reserved, connected, verified, and committed.

The result is a laboratory architecture where logical workflow and state are stable, while the actual physical provisioning path remains a dynamic, simulated laboratory process.