from __future__ import annotations
import json
import os
from uuid import uuid4
import psycopg
from psycopg.rows import dict_row
from psycopg.types.json import Json
from engine.resource_engine import ResourceRegistry
from smart_cabinet.models import EventType, SourceMode, TelemetryEvent, led_for_known_state

def connect() -> psycopg.Connection:
    return psycopg.connect(host=os.environ["FIBERSPIDER_DB_HOST"], port=os.environ["FIBERSPIDER_DB_PORT"], dbname=os.environ["FIBERSPIDER_DB_NAME"], user=os.environ["FIBERSPIDER_DB_USER"], password=os.environ["FIBERSPIDER_DB_PASSWORD"], row_factory=dict_row)

def agent_proposal(event: TelemetryEvent) -> dict[str, object]:
    """Repeatable stand-in for an advisory LLM call; never authorizes a change."""
    return {"component": "cabinet-agent", "mode": "SIMULATED_ADVISORY", "decision": "PROPOSE_RESERVATION", "resource_id": event.port_id, "reason": "ONT observed on a known splitter port with acceptable optical level", "authoritative": False}

def deterministic_validate(event: TelemetryEvent, proposal: dict[str, object], authorized: bool) -> dict[str, object]:
    reasons: list[str] = []
    if not authorized: reasons.append("technician is not active")
    if not event.ont_present or not event.gpon_serial: reasons.append("ONT evidence is incomplete")
    if not event.port_id or not event.splitter_id: reasons.append("port/splitter context is incomplete")
    if event.optical_rx_dbm is None or not -28.0 <= event.optical_rx_dbm <= -8.0: reasons.append("optical level is outside demo policy")
    if event.anomaly_code: reasons.append("anomaly blocks reservation")
    if proposal.get("decision") != "PROPOSE_RESERVATION": reasons.append("proposal type is not allowed")
    return {"validator": "deterministic-demo-policy-v1", "authorized": not reasons, "reasons": reasons}

def run() -> dict[str, object]:
    technician_id = "TECH-DEMO-001"
    event = TelemetryEvent(cabinet_id="CAB-A", event_type=EventType.ONT_PRESENCE, source_mode=SourceMode.SIMULATED, ont_present=True, gpon_serial="FS-DEMO-ONT-0001", splitter_id="SPL-A-01", port_id="PORT-A-01", optical_rx_dbm=-19.4, current_state="DETECTED", technician_id=technician_id, cabinet_open=True)
    event_record = event.as_record()
    with connect() as conn:
        with conn.cursor() as cur:
            cur.execute("SELECT status FROM technicians WHERE technician_id = %s", (technician_id,))
            technician = cur.fetchone()
            proposal = agent_proposal(event)
            validation = deterministic_validate(event, proposal, bool(technician and technician["status"] == "ACTIVE"))
            if not validation["authorized"]: raise RuntimeError(f"deterministic validation rejected proposal: {validation['reasons']}")
            cur.execute("SELECT state FROM demo_resources WHERE resource_id = %s FOR UPDATE", (event.port_id,))
            resource = cur.fetchone()
            if not resource or resource["state"] != "FREE": raise RuntimeError("resource is not available")
            registry = ResourceRegistry()
            registry.register_resource(event.port_id, "PORT", cabinet_id=event.cabinet_id, splitter_id=event.splitter_id)
            transaction = registry.reserve_resource(event.port_id, technician_id, ont_serial=event.gpon_serial)
            transaction_id = transaction["transaction_id"]
            cur.execute("UPDATE demo_resources SET state = 'RESERVED', owner_transaction_id = %s, updated_at = now() WHERE resource_id = %s", (transaction_id, event.port_id))
            cur.execute("INSERT INTO telemetry_events (event_id, cabinet_id, event_type, observed_at, payload, source_mode) VALUES (%s, %s, %s, %s, %s, %s)", (event.event_id, event.cabinet_id, event.event_type.value, event.observed_at, Json(event_record), event.source_mode.value))
            run_id = f"RUN-{uuid4().hex[:12].upper()}"
            cur.execute("INSERT INTO demo_runs (run_id, technician_id, event_id, agent_proposal, validation_result, transaction_id, final_state) VALUES (%s, %s, %s, %s, %s, %s, 'RESOURCE_RESERVED')", (run_id, technician_id, event.event_id, Json(proposal), Json(validation), transaction_id))
        conn.commit()
    led = led_for_known_state("RESERVED")
    result = {"demo": "FiberSpider reproducible R&D flow", "technician": {"id": technician_id, "authenticated": True}, "telemetry": event_record, "agent_proposal": proposal, "deterministic_validation": validation, "reservation": {"resource_id": event.port_id, "transaction_id": transaction_id, "state": "RESERVED", "validated_by": transaction["validated_by"]}, "postgresql": {"updated": True, "run_id": run_id}, "diagnostic_led": {"led_id": led.led_id, "state": led.state.value, "reason": led.reason}, "truth_status": {"software": "WORKING", "telemetry_input": "SIMULATED", "agent": "SIMULATED_ADVISORY", "physical_provisioning": "NOT_IMPLEMENTED"}}
    print(json.dumps(result, indent=2, default=str))
    return result

if __name__ == "__main__": run()
