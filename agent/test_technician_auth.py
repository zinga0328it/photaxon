from __future__ import annotations

from agent.cabinet_agent import CabinetAgent, Event
from engine.postgres_store import PostgresStore


class MockPostgresStore(PostgresStore):
    def __init__(self, authorized_technicians: set[str]) -> None:
        self.authorized_technicians = authorized_technicians

    def connect(self) -> None:
        pass

    def is_technician_authorized(self, technician_id: str) -> bool:
        return technician_id in self.authorized_technicians


def test_auth_without_id() -> bool:
    agent = CabinetAgent(agent_id="AGENT-CAB-A", cabinet_id="CAB-A", store=MockPostgresStore(set()))
    event = Event(type="TECHNICIAN_ARRIVED", payload={})
    result = agent.handle_event(event, dry_run=False)
    return result["proposal"]["decision"] == "AUTENTICAZIONE_RICHIESTA"


def test_unknown_technician() -> bool:
    agent = CabinetAgent(agent_id="AGENT-CAB-A", cabinet_id="CAB-A", store=MockPostgresStore(set()))
    event = Event(type="TECHNICIAN_ARRIVED", payload={"technician_id": "3512424187"})
    result = agent.handle_event(event, dry_run=False)
    return result["proposal"]["decision"] == "ACCESS_DENIED"


def test_valid_technician() -> bool:
    agent = CabinetAgent(agent_id="AGENT-CAB-A", cabinet_id="CAB-A", store=MockPostgresStore({"3512424187"}))
    event = Event(type="TECHNICIAN_ARRIVED", payload={"technician_id": "3512424187"})
    result = agent.handle_event(event, dry_run=False)
    return result["proposal"]["decision"] == "AUTHENTICATED"


if __name__ == "__main__":
    tests = [
        ("AUTH WITHOUT ID", test_auth_without_id()),
        ("UNKNOWN TECHNICIAN", test_unknown_technician()),
        ("VALID TECHNICIAN", test_valid_technician()),
    ]
    for name, passed in tests:
        print(f"{name}: {'PASS' if passed else 'FAIL'}")
    raise SystemExit(0 if all(passed for _, passed in tests) else 1)
