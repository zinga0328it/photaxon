from __future__ import annotations
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from enum import Enum
from typing import Any
from uuid import uuid4

class EventType(str, Enum):
    ONT_PRESENCE = "ONT_PRESENCE"
    GPON_SERIAL_OBSERVED = "GPON_SERIAL_OBSERVED"
    PORT_STATE_CHANGED = "PORT_STATE_CHANGED"
    OPTICAL_LEVEL_REPORTED = "OPTICAL_LEVEL_REPORTED"
    ANOMALY_DETECTED = "ANOMALY_DETECTED"
    CABINET_OPENED = "CABINET_OPENED"
    TECHNICIAN_INTERVENTION = "TECHNICIAN_INTERVENTION"
    LED_STATE_CHANGED = "LED_STATE_CHANGED"

class SourceMode(str, Enum):
    SIMULATED = "SIMULATED"
    PHYSICAL = "PHYSICAL"

class LedState(str, Enum):
    OFF = "OFF"
    GREEN = "GREEN"
    AMBER = "AMBER"
    RED = "RED"
    BLINKING_BLUE = "BLINKING_BLUE"

@dataclass(frozen=True)
class DiagnosticLed:
    """Output derived from known state; it is not a sensor."""
    led_id: str
    state: LedState
    reason: str

@dataclass(frozen=True)
class TelemetryEvent:
    cabinet_id: str
    event_type: EventType
    source_mode: SourceMode
    ont_present: bool | None = None
    gpon_serial: str | None = None
    splitter_id: str | None = None
    port_id: str | None = None
    optical_rx_dbm: float | None = None
    previous_state: str | None = None
    current_state: str | None = None
    anomaly_code: str | None = None
    technician_id: str | None = None
    cabinet_open: bool | None = None
    metadata: dict[str, Any] = field(default_factory=dict)
    event_id: str = field(default_factory=lambda: f"EVT-{uuid4().hex[:12].upper()}")
    observed_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))

    def validate(self) -> None:
        if self.event_type in {EventType.ONT_PRESENCE, EventType.GPON_SERIAL_OBSERVED} and not self.gpon_serial:
            raise ValueError("gpon_serial is required for ONT/GPON events")
        if self.event_type == EventType.OPTICAL_LEVEL_REPORTED and self.optical_rx_dbm is None:
            raise ValueError("optical_rx_dbm is required for optical-level events")
        if self.port_id and not self.splitter_id:
            raise ValueError("splitter_id is required when port_id is present")

    def as_record(self) -> dict[str, Any]:
        self.validate()
        record = asdict(self)
        record["event_type"] = self.event_type.value
        record["source_mode"] = self.source_mode.value
        record["observed_at"] = self.observed_at.isoformat()
        return record

def led_for_known_state(resource_state: str, anomaly_code: str | None = None) -> DiagnosticLed:
    if anomaly_code or resource_state == "FAULT":
        return DiagnosticLed("PORT-DIAG", LedState.RED, anomaly_code or "resource fault")
    if resource_state == "FREE":
        return DiagnosticLed("PORT-DIAG", LedState.GREEN, "port available")
    if resource_state in {"RESERVED", "CONNECTED", "VERIFIED"}:
        return DiagnosticLed("PORT-DIAG", LedState.AMBER, f"workflow state {resource_state}")
    if resource_state == "OCCUPIED":
        return DiagnosticLed("PORT-DIAG", LedState.BLINKING_BLUE, "active known allocation")
    return DiagnosticLed("PORT-DIAG", LedState.OFF, "state unavailable")

