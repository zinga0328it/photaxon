# FiberSpider Branches

This repository uses branch-based public nodes for the FiberSpider laboratory:

- `pc-ragno`: public branch for the RAGNO laboratory node, containing the FiberSpider backend, activation API, and laboratory simulation.
- `pc-aaa`: public branch for the AAA persistent database node, documenting PostgreSQL persistence and state management.
- `pc-alex`: public branch for the ALEX public frontend node, documenting the Apache reverse proxy and public demo interface.

## Branch roles

- `pc-ragno` is the current active lab branch and the working branch for this documentation.
- `pc-aaa` describes the persistent PostgreSQL state node for FiberSpider.
- `pc-alex` describes the public frontend/reverse proxy node for FiberSpider.

## Public branch guidance

- Do not merge, rebase, or cherry-pick between these branches in this public documentation workflow.
- Each branch is intended to describe a separate node role in the FiberSpider laboratory.
- `pc-ragno` focuses on backend logic, activation flow, and laboratory simulation.
- `pc-aaa` focuses on authoritative storage and state persistence.
- `pc-alex` focuses on the public interface and frontend proxy.

## Notes

This branch does not modify or publish operational configuration from the main laboratory environment. It is a carefully curated public snapshot of the FiberSpider proof-of-concept architecture.