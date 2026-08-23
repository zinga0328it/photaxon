from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Any


CABINET = "CAB-A"
LOCAL_AGENT = "AGENT-CAB-A"
TECHNICIAN_ID = "1234567890"
ONT_SERIAL = "FS-ONT-DEMO-0001"
ONT_VENDOR = "DEMO-ONT"
MODEM_MODEL = "FS-CPE-DEMO-01"
PEER_CABS = ["CAB-B", "CAB-C"]


def build_prompt() -> str:
    return (
        "Sei un agente FiberSpider di CAB-A. Ricevi un evento di tipo ONT_DETECTED.\n"
        "Non modificare alcuno stato reale, non acquisire lease, non eseguire provisioning, non toccare DB o OpenStack.\n"
        "Devi produrre solo una PROPOSTA.\n"
        "\n"
        "CONTESTO:\n"
        f"- cabinet_id: {CABINET}\n"
        f"- agent_id: {LOCAL_AGENT}\n"
        f"- technician_id: {TECHNICIAN_ID}\n"
        "- technician authenticated: yes\n"
        f"- ont_serial: {ONT_SERIAL}\n"
        f"- ont_vendor: {ONT_VENDOR}\n"
        f"- modem_model: {MODEM_MODEL}\n"
        "- physical_port: unknown\n"
        "- topology known: CENTRAL-01 with peer cabinets CAB-A, CAB-B, CAB-C\n"
        "- CAB-A resources: no physical port assigned, ONT detected, modem detected\n"
        "- CAB-B and CAB-C are available peers\n"
        "- cannot invent a physical port\n"
        "- cannot choose another splitter than declared by the technician\n"
        "\n"
        "Rispondi esattamente con le seguenti sezioni:\n"
        "OBSERVATION:\n"
        "PROPOSAL:\n"
        "VALIDATION:\n"
        "NEXT_ACTION:\n"
        "MISSING_INFORMATION:\n"
        "\n"
        "Se manca un dato necessario, indica esplicitamente cosa manca.\n"
    )


def call_codex(prompt: str) -> str:
    codex_path = Path("/usr/local/bin/codex")
    if not codex_path.exists():
        raise FileNotFoundError("Codex runtime not found at /usr/local/bin/codex")

    command = [str(codex_path), "exec", "--ephemeral", "--sandbox", "read-only"]
    result = subprocess.run(
        command,
        input=prompt,
        capture_output=True,
        text=True,
        check=False,
        timeout=45,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Codex failed: {result.returncode} {result.stderr.strip()}")
    return result.stdout.strip()


def parse_proposal(output: str) -> dict[str, str]:
    sections = {
        "OBSERVATION": "",
        "PROPOSAL": "",
        "VALIDATION": "",
        "NEXT_ACTION": "",
        "MISSING_INFORMATION": "",
    }
    current = None
    for line in output.splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        if ":" in stripped:
            key, value = stripped.split(":", 1)
            key = key.strip().upper()
            if key in sections:
                current = key
                sections[key] = value.strip()
                continue
        if current:
            sections[current] += " " + stripped
    return sections


def validate_proposal(sections: dict[str, str]) -> tuple[str, str]:
    next_action = sections["NEXT_ACTION"].upper()
    proposal = sections["PROPOSAL"].lower()
    missing = sections["MISSING_INFORMATION"].strip()

    if "physical port" in next_action.lower() or "request" in next_action.lower() or "wait" in next_action.lower():
        if "physical_port" in proposal or "port" in proposal or "splitter" in proposal:
            return "PASS", missing or ""
        return "PASS", missing or "physical port declaration needed"
    if "reserve" in next_action.lower() or "provision" in next_action.lower() or "acquire" in next_action.lower():
        return "FAIL", "proposal attempts resource action without confirmed physical port"
    if not next_action:
        return "INCOMPLETE", "missing next action"
    return "INCOMPLETE", missing or "unclear next action"


def main() -> None:
    report: dict[str, str] = {
        "CABINET": CABINET,
        "ONT": ONT_SERIAL,
        "LOCAL AGENT": LOCAL_AGENT,
        "PEERS AVAILABLE": ", ".join(PEER_CABS),
        "LLM CALLED": "NO",
        "PROPOSAL CREATED": "NO",
        "PROPOSED PATH": "",
        "BACKEND VALIDATION": "INCOMPLETE",
        "MISSING INFORMATION": "",
        "NEXT ACTION": "",
        "LEASE ACQUIRED": "NO",
        "PROVISIONING EXECUTED": "NO",
        "DB MODIFIED": "NO",
        "OPENSTACK MODIFIED": "NO",
    }

    prompt = build_prompt()
    try:
        output = call_codex(prompt)
        report["LLM CALLED"] = "YES"
    except Exception as exc:
        report["MISSING INFORMATION"] = f"LLM call failed: {exc}"
        print_report(report)
        return

    sections = parse_proposal(output)
    report["PROPOSAL CREATED"] = "YES" if sections["PROPOSAL"] else "NO"
    report["PROPOSED PATH"] = sections["PROPOSAL"] or ""
    validation, missing = validate_proposal(sections)
    report["BACKEND VALIDATION"] = validation
    report["MISSING INFORMATION"] = missing
    report["NEXT ACTION"] = sections["NEXT_ACTION"] or ""

    print_report(report)


def print_report(report: dict[str, str]) -> None:
    for key, value in report.items():
        print(f"{key}: {value}")


if __name__ == "__main__":
    main()
