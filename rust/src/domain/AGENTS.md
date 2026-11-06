# FiberSpider Domain Models

## Purpose
Define typed FTTH resources and event payloads only: cabinets, lines, splitters, ROE/PTE, ONT, paths, and anomalies.

## Authorized Files
`src/domain/**`; no SQL, HTTP, process, or OS code.

## Allowed
Pure data types, serde/schema validation metadata, and small deterministic unit tests.

## Forbidden
Database access, Codex invocation, filesystem/shell, network configuration, secrets, or side effects in deserialization.

## Test and Done
Invalid identifiers/values have focused tests; models contain no credential values and do not duplicate repository state.
