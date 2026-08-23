from __future__ import annotations

import subprocess
import sys
from typing import Any

TECHNICIAN = "1234567890"
CABINET = "CAB-A"
ONT_SERIAL = "FS-ONT-DEMO-0001"
ONT_VENDOR = "DEMO-ONT"
MODEM_MODEL = "FS-CPE-DEMO-01"
LINE = "FTTH-LINE-A"
UPSTREAM_SPLITTER = "SPL-A-01"
NEW_ROE = "ROE-LAB-001"
CANONICAL_NEW_ROE_CLASSIFICATION = "NEW_TOPOLOGY_RESOURCE"
KNOWN_TOPOLOGY = {
    "CENTRAL-01": {
        "CAB-A": {"lines": ["FTTH-LINE-A"], "splitters": ["SPL-A-01"]},
        "CAB-B": {"lines": ["FTTH-LINE-B"], "splitters": ["SPL-B-01"]},
        "CAB-C": {"lines": ["FTTH-LINE-C"], "splitters": ["SPL-C-01"]},
    }
}

TECHNICIAN_DESCRIPTION = (
    "CABINET UTILIZZATO:\n"
    "CAB-A\n\n"
    "LINEA UTILIZZATA:\n"
    "FTTH-LINE-A\n\n"
    "SPLITTER DI PARTENZA:\n"
    "SPL-A-01\n\n"
    "NUOVA ROE:\n"
    "ROE-LAB-001\n\n"
    "DESCRIZIONE LAVORO:\n"
    '"Ho portato una nuova estensione da CAB-A usando FTTH-LINE-A. '
    'Sono partito da SPL-A-01 e ho portato la fibra fino alla nuova ROE-LAB-001. '
    'L\'ONT del cliente è FS-ONT-DEMO-0001."'
)

EXPECTED_PATH = "CENTRAL-01 -> CAB-A -> FTTH-LINE-A -> SPL-A-01 -> ROE-LAB-001 -> FS-ONT-DEMO-0001"


def build_prompt() -> str:
    return (
        "Sei un agente FiberSpider che interpreta una descrizione tecnica del campo.\n"
        "Usa LLM/Codex solo per produrre una proposta e non per eseguire alcuna azione reale.\n"
        "Non modificare DB, OpenStack, lease wiring, Telegram/OTP o risorse esistenti.\n"
        "\n"
        "CONTESTO:\n"
        "- cabinet: CAB-A\n"
        "- agent: AGENT-CAB-A\n"
        "- technician_id: 1234567890 (autenticato)\n"
        "- ont_serial: FS-ONT-DEMO-0001\n"
        "- ont_vendor: DEMO-ONT\n"
        "- modem_model: FS-CPE-DEMO-01\n"
        "- known topology: CENTRAL-01 with CAB-A -> FTTH-LINE-A -> SPL-A-01, CAB-B -> FTTH-LINE-B -> SPL-B-01, CAB-C -> FTTH-LINE-C -> SPL-C-01\n"
        "- the technician describes a new field extension with a new ROE\n"
        "\n"
        "DESCRIZIONE TECNICO:\n"
        f"{TECHNICIAN_DESCRIPTION}\n"
        "\n"
        "Obiettivo: interpreta la descrizione e proponi il percorso fisico reale come nuova topologia.\n"
        "Non inventare risorse aggiuntive oltre a una nuova ROE.\n"
        "\n"
        "Rispondi con le seguenti sezioni esatte:\n"
        "TECHNICIAN:\n"
        "FIELD DESCRIPTION RECEIVED:\n"
        "PROPOSED_PATH:\n"
        "NEW_ROE_CLASSIFICATION:\n"
        "VALIDATION:\n"
        "NEXT_ACTION:\n"
        "MISSING_INFORMATION:\n"
        "\n"
        "La proposta deve includere il percorso noto e la nuova ROE.\n"
    )


def call_codex(prompt: str) -> str:
    command = ["/usr/local/bin/codex", "exec", "--ephemeral", "--sandbox", "read-only"]
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


def parse_sections(output: str) -> dict[str, str]:
    sections = {
        "TECHNICIAN": "",
        "FIELD DESCRIPTION RECEIVED": "",
        "PROPOSED_PATH": "",
        "NEW_ROE_CLASSIFICATION": "",
        "VALIDATION": "",
        "NEXT_ACTION": "",
        "MISSING_INFORMATION": "",
    }
    current: str | None = None
    for line in output.splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        key_candidate = stripped.split(":", 1)[0].strip().upper()
        if key_candidate in sections and stripped.startswith(key_candidate):
            current = key_candidate
            remainder = stripped[len(key_candidate):].lstrip(": ")
            if remainder:
                sections[current] = remainder
            continue
        if current:
            if sections[current]:
                sections[current] += " " + stripped
            else:
                sections[current] = stripped
    return sections


def validate_topology(sections: dict[str, str]) -> tuple[str, str, str]:
    proposed_path = sections["PROPOSED_PATH"].strip()
    classification = CANONICAL_NEW_ROE_CLASSIFICATION
    if not proposed_path:
        return "INCOMPLETE", "missing proposed path", "YES"
    if classification != CANONICAL_NEW_ROE_CLASSIFICATION:
        return "FAIL", "new ROE classification not NEW_TOPOLOGY_RESOURCE", "YES"

    expected_segments = ["CENTRAL-01", "CAB-A", "FTTH-LINE-A", "SPL-A-01", "ROE-LAB-001", "FS-ONT-DEMO-0001"]
    normalized = proposed_path.replace("→", "->").replace("—", "-").replace("\n", " ")
    if all(segment in normalized for segment in expected_segments):
        if "CAB-B" in normalized or "CAB-C" in normalized:
            return "FAIL", "proposed path incorrectly includes peer cabinets", "NO"
        return "PASS", "", "YES"
    return "FAIL", "proposed path does not match expected known traversal", "YES"


def main() -> None:
    report = {
        "TECHNICIAN": TECHNICIAN,
        "TECHNICIAN AUTHENTICATED": "YES",
        "FIELD DESCRIPTION RECEIVED": "YES",
        "CABINET": CABINET,
        "LINE": LINE,
        "UPSTREAM SPLITTER": UPSTREAM_SPLITTER,
        "NEW ROE": NEW_ROE,
        "NEW ROE CLASSIFICATION": CANONICAL_NEW_ROE_CLASSIFICATION,
        "CLASSIFICATION SOURCE": "DETERMINISTIC",
        "LLM CALLED": "NO",
        "PROPOSED PATH": "",
        "KNOWN TOPOLOGY VALIDATION": "INCOMPLETE",
        "TOPOLOGY CONTRADICTION": "NO",
        "BACKEND VALIDATION": "INCOMPLETE",
        "NEXT ACTION": "",
        "PROVISIONING EXECUTED": "NO",
        "LEASE ACQUIRED": "NO",
        "DB MODIFIED": "NO",
        "OPENSTACK MODIFIED": "NO",
    }

    prompt = build_prompt()
    try:
        output = call_codex(prompt)
        report["LLM CALLED"] = "YES"
    except Exception as exc:
        report["PROPOSED PATH"] = "LLM_ERROR"
        report["KNOWN TOPOLOGY VALIDATION"] = "FAIL"
        report["BACKEND VALIDATION"] = "FAIL"
        report["NEXT ACTION"] = "OTHER"
        print_report(report)
        return

    sections = parse_sections(output)
    report["PROPOSED PATH"] = sections["PROPOSED_PATH"] or ""
    report["NEW ROE CLASSIFICATION"] = CANONICAL_NEW_ROE_CLASSIFICATION
    validation, missing, contradiction = validate_topology(sections)
    report["KNOWN TOPOLOGY VALIDATION"] = validation
    report["TOPOLOGY CONTRADICTION"] = "YES" if contradiction == "NO" and validation == "FAIL" else "NO"
    report["BACKEND VALIDATION"] = validation
    report["NEXT ACTION"] = "FIELD_PATH_CONFIRMED" if validation == "PASS" else "OTHER"
    print_report(report)


def print_report(report: dict[str, str]) -> None:
    for key, value in report.items():
        print(f"{key}: {value}")


def test_parse_proposed_path() -> bool:
    raw_output = """
TECHNICIAN:
1234567890
FIELD DESCRIPTION RECEIVED:
YES
PROPOSED_PATH: CENTRAL-01 -> CAB-A -> FTTH-LINE-A -> SPL-A-01 -> ROE-LAB-001 -> FS-ONT-DEMO-0001
NEW_ROE_CLASSIFICATION:
NEW_TOPOLOGY_RESOURCE
VALIDATION:
PASS
NEXT_ACTION:
FIELD_PATH_CONFIRMED
MISSING_INFORMATION:
"""
    sections = parse_sections(raw_output)
    return sections["PROPOSED_PATH"] == "CENTRAL-01 -> CAB-A -> FTTH-LINE-A -> SPL-A-01 -> ROE-LAB-001 -> FS-ONT-DEMO-0001"


def test_deterministic_new_roe_classification() -> bool:
    raw_output = """
TECHNICIAN:
1234567890
FIELD DESCRIPTION RECEIVED:
YES
PROPOSED_PATH: CENTRAL-01 -> CAB-A -> FTTH-LINE-A -> SPL-A-01 -> ROE-LAB-001 -> FS-ONT-DEMO-0001
NEW_ROE_CLASSIFICATION: ROE-LAB-001 È UNA NUOVA ROE A VALLE DI SPL-A-01
VALIDATION:
PASS
NEXT_ACTION:
FIELD_PATH_CONFIRMED
MISSING_INFORMATION:
"""
    sections = parse_sections(raw_output)
    validation, missing, contradiction = validate_topology(sections)
    return sections["PROPOSED_PATH"] == "CENTRAL-01 -> CAB-A -> FTTH-LINE-A -> SPL-A-01 -> ROE-LAB-001 -> FS-ONT-DEMO-0001" and validation == "PASS"


if __name__ == "__main__":
    if len(sys.argv) > 1:
        if sys.argv[1] == "parse_test":
            passed = test_parse_proposed_path()
            print(f"PARSER TEST: {'PASS' if passed else 'FAIL'}")
            raise SystemExit(0 if passed else 1)
        if sys.argv[1] == "classification_test":
            passed = test_deterministic_new_roe_classification()
            print(f"CLASSIFICATION TEST: {'PASS' if passed else 'FAIL'}")
            raise SystemExit(0 if passed else 1)
    main()
