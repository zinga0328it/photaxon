# FiberSpider architecture diagram

```mermaid
flowchart TD
    B["Browser / technician"] -->|HTTPS| A["ALEX<br/>public frontend + reverse proxy"]
    A -->|API| R["RAGNO<br/>backend + agents + laboratory"]
    R -->|authoritative state| D["AAA<br/>PostgreSQL"]
    C1["CAB-A<br/>smart cabinet node"] <--> C2["CAB-B<br/>smart cabinet node"]
    C2 <--> C3["CAB-C<br/>smart cabinet node"]
    R -.->|validated coordination| C2
```

```mermaid
flowchart LR
    T["Telemetry event"] --> L["LLM agent proposal"]
    L --> V["Deterministic validation"]
    V -->|authorized| E["Resource engine"]
    V -->|rejected| X["Audit / no action"]
    E --> P["PostgreSQL state"]
```

The LLM agent is advisory. It cannot reserve ports, change topology, provision an
ONT or update authoritative state directly. Only deterministic code performs an
allowed transition after technician authorization and evidence validation.

