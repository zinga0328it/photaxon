# Smart Cabinet / Telemetry

This module defines a vendor-neutral software contract for future cabinet sensors.
It does **not** claim that physical sensors are installed or connected today.

The `smart_cabinet` package models ONT presence, GPON serial observations,
splitter/port context, optical receive level, state changes, anomalies, cabinet
opening, technician intervention and diagnostic LED state changes. Every event
declares `source_mode` as `SIMULATED` or `PHYSICAL`; current public demo events
are always `SIMULATED`.

## Sensors and diagnostic LEDs are different

A sensor produces evidence. A diagnostic LED is an output controlled from state
already known by the node. It never proves that a physical condition occurred.

| Known state | LED | Meaning |
|---|---|---|
| `FREE` | green | port available |
| `RESERVED`, `CONNECTED`, `VERIFIED` | amber | workflow in progress |
| `OCCUPIED` | blinking blue | active known allocation |
| `FAULT` or anomaly | red | attention required |
| unknown | off | no authoritative indication |

These colors are a laboratory policy, not a telecom standard.

