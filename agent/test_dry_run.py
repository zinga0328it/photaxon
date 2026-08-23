from __future__ import annotations

from .cabinet_agent import CabinetAgent, Event


def run_dry_run() -> bool:
    agent = CabinetAgent(agent_id="AGENT-CAB-A", cabinet_id="CAB-A")
    event = Event(type="TECHNICIAN_ARRIVED", payload={"location": "door", "time": "now"})
    result = agent.handle_event(event, dry_run=True)
    return result["action"] == "dry_run" and result["proposal"] is not None


if __name__ == "__main__":
    success = run_dry_run()
    print("DRY RUN PASS" if success else "DRY RUN FAIL")
    raise SystemExit(0 if success else 1)
