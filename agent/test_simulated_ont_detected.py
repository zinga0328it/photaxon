from __future__ import annotations

from agent.cabinet_agent import Event


TECHNICIAN_ID = "1234567890"
DISPLAY_NAME = "Technician"


def simulate_ont_detected() -> dict[str, str]:
    authenticated_technician = {
        "technician_id": TECHNICIAN_ID,
        "display_name": DISPLAY_NAME,
        "authenticated": True,
    }

    event = Event(
        type="ONT_DETECTED",
        payload={
            "technician_id": TECHNICIAN_ID,
            "ont_serial": "FS-ONT-DEMO-0001",
            "ont_vendor": "DEMO-ONT",
            "modem_model": "FS-CPE-DEMO-01",
            # porta fisica non dichiarata
        },
    )

    technician_authenticated = (
        authenticated_technician.get("authenticated") is True
        and authenticated_technician.get("technician_id") == TECHNICIAN_ID
    )

    ont_detected = bool(event.payload.get("ont_serial") or event.payload.get("ont_vendor"))
    modem_detected = bool(event.payload.get("modem_model"))
    physical_port = event.payload.get("physical_port")

    decision = "WAIT"
    next_action = "REQUEST_PHYSICAL_PORT"
    if not (technician_authenticated and ont_detected and modem_detected and not physical_port):
        decision = "OTHER"
        next_action = "OTHER"

    return {
        "CABINET": "CAB-A",
        "TECHNICIAN AUTHENTICATED": "YES" if technician_authenticated else "NO",
        "ONT DETECTED": "YES" if ont_detected else "NO",
        "ONT SERIAL": event.payload.get("ont_serial", ""),
        "MODEM DETECTED": "YES" if modem_detected else "NO",
        "PHYSICAL PORT": "UNKNOWN" if not physical_port else str(physical_port),
        "DECISION": decision,
        "NEXT ACTION": next_action,
        "PROVISIONING EXECUTED": "NO",
        "PORT INVENTED": "NO",
        "DB MODIFIED": "NO",
        "OPENSTACK MODIFIED": "NO",
    }


if __name__ == "__main__":
    report = simulate_ont_detected()
    for key, value in report.items():
        print(f"{key}: {value}")
