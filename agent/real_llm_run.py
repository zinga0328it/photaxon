from __future__ import annotations

from .cabinet_agent import CabinetAgent, Event
from .llm_runtime import LLMRuntime


def run_real_llm() -> dict[str, object]:
    agent = CabinetAgent(
        agent_id="AGENT-CAB-A",
        cabinet_id="CAB-A",
        llm_runtime=LLMRuntime(codex_path="/usr/local/bin/codex"),
    )

    event = Event(
        type="TECHNICIAN_ARRIVED",
        payload={
            "request_id": "SIM-FTTH-001",
            "technician_id": "TECH-DEMO-01",
            "cabinet_id": "CAB-A",
        },
    )

    result = agent.handle_event(event, dry_run=False)
    return result


if __name__ == "__main__":
    output = run_real_llm()
    print(output)
