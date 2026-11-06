# FTTH Topology

## Purpose
Validate the dynamic physical graph linking CENTRAL-01, cabinets, lines, splitters, ROE/PTE, ports, and ONT.

## Authorized Files
Topology DTO/validator/service files only. Current path DTO is in `src/domain/topology.rs`; coordinate future moves with that module.

## Allowed
Deterministic path and relationship validation; produce explicit validation results for services and MCP tools.

## Forbidden
Direct database mutations, Codex calls, shell, OpenStack/Neutron control, or host networking changes.

## Test and Done
Valid CAB→line→splitter paths pass; missing, cross-cabinet, duplicate, or impossible links reject; every persisted topology change is audited by the DB service.
