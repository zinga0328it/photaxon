from __future__ import annotations

import json
import subprocess
import tempfile
from unittest.mock import patch

from agent.cabinet_agent import CabinetAgent
from agent.decision_worker import evaluate as evaluate_worker
from agent.llm_runtime import LLMRuntime


def test_codex_stderr_diagnostics_no_command_detection() -> bool:
    stdout = "DECISION: ATTESA\nNEXT_ACTION: IDENTIFICARE ONT/MODEM E PORTA FISICA\nREASON: APPARATO E PORTA NON RILEVATI; PROVISIONING NON AUTORIZZATO"
    stderr = (
        "Reading prompt from stdin...\n"
        "OpenAI Codex v0.149.0\n"
        "--------\n"
        "workdir: /path/to/fiberspider\n"
        "model: gpt-5.6-sol\n"
        "provider: openai\n"
        "approval: never\n"
        "sandbox: read-only\n"
        "reasoning effort: none\n"
        "reasoning summaries: none\n"
        "session id: 01a02a2c-ee37-71b2-bd5f-6fb0d4888c52\n"
        "--------\n"
        "user\n"
        "Sei l'agente consultivo dell'armadio FTTH CAB-A.\n"
        "NON eseguire comandi.\n"
        "NON leggere filesystem.\n"
        "NON modificare rete.\n"
        "NON modificare database.\n"
        "NON modificare OpenStack.\n"
        "Devi soltanto valutare l'evento ricevuto.\n\n"
        "EVENTO:\nTECHNICIAN_ARRIVED\n\n"
        "CONTESTO:\nrequest_id=SIM-FTTH-001\n"
        "technician_id=TECH-DEMO-01\n"
        "cabinet_id=CAB-A\n"
        "nessun ONT/modem ancora rilevato.\n"
        "nessuna porta ancora scelta.\n"
        "nessun provisioning ancora autorizzato.\n\n"
        "Per TECHNICIAN_ARRIVED, se mancano identificazione apparato e porta fisica, NON proporre provisioning.\n\n"
        "Rispondi brevissimamente con SOLO:\n"
        "DECISION:\nNEXT_ACTION:\nREASON:\n"
        "warning: Codex's Linux sandbox uses bubblewrap and needs access to create user namespaces.\n"
        "codex\n"
        "DECISION: ATTESA\n"
        "NEXT_ACTION: IDENTIFICARE ONT/MODEM E PORTA FISICA\n"
        "REASON: APPARATO E PORTA NON RILEVATI; PROVISIONING NON AUTORIZZATO\n"
        "tokens used 8,503"
    )

    runtime = LLMRuntime()
    commands_detected = runtime._contains_disallowed_terms(stdout) or runtime._contains_disallowed_terms(stderr, ignore_codex_diagnostics=True)
    fields = {
        "DECISION": "ATTESA",
        "NEXT_ACTION": "IDENTIFICARE ONT/MODEM E PORTA FISICA",
        "REASON": "APPARATO E PORTA NON RILEVATI; PROVISIONING NON AUTORIZZATO",
    }
    return commands_detected is False and all(fields.values())


def _mock_decision(decision: str) -> bool:
    context = {
        "candidate_cabinet_id": "CAB-D",
        "current_roe_id": "ROE-OLD",
        "candidate_roe_id": "ROE-NEW",
        "candidate_splitter_id": "SPL-D-01",
        "distance_m": 32.5,
        "available_fibers": 4,
        "required_fibers": 1,
        "inventory_fingerprint": "a" * 64,
    }
    response = {
        "decision": decision,
        "agent_id": "AGENT-CAB-D",
        "cabinet_id": "CAB-D",
        "current_roe_id": "ROE-OLD",
        "candidate_roe_id": "ROE-NEW",
        "inventory_fingerprint": "a" * 64,
        "reason_code": "TEST_DECISION",
        "risk": "LOW" if decision == "ALLOW" else "HIGH",
    }
    completed = subprocess.CompletedProcess(
        args=["codex"],
        returncode=0,
        stdout=json.dumps(response),
        stderr="",
    )
    agent = CabinetAgent(
        agent_id="AGENT-CAB-D",
        cabinet_id="CAB-D",
        llm_runtime=LLMRuntime(codex_path="mock-codex"),
    )
    with patch("agent.llm_runtime.subprocess.run", return_value=completed):
        result = agent.evaluate_roe_selection(context)
    return result.get("success") is True and result.get("decision") == decision


def test_roe_decision_allow_is_structured_and_mocked() -> bool:
    return _mock_decision("ALLOW")


def test_roe_decision_deny_is_structured_and_mocked() -> bool:
    return _mock_decision("DENY")


def test_roe_decision_wrong_cabinet_fails_closed() -> bool:
    agent = CabinetAgent(agent_id="AGENT-CAB-A", cabinet_id="CAB-A")
    result = agent.evaluate_roe_selection({"candidate_cabinet_id": "CAB-D"})
    return result == {"success": False, "error": "selected cabinet mismatch"}


def test_worker_routes_allow_and_deny_to_the_selected_cabinet_agent() -> bool:
    context = {
        "candidate_cabinet_id": "CAB-D",
        "current_roe_id": "ROE-OLD",
        "candidate_roe_id": "ROE-NEW",
        "candidate_splitter_id": "SPL-D-01",
        "distance_m": 32.5,
        "available_fibers": 4,
        "required_fibers": 1,
        "inventory_fingerprint": "a" * 64,
    }
    for decision in ("ALLOW", "DENY"):
        with patch("agent.decision_worker.LLMRuntime") as runtime_type:
            runtime = runtime_type.return_value
            runtime.roe_selection_decision.return_value = {
                "success": True,
                "decision": decision,
                "agent_id": "AGENT-CAB-D",
                "cabinet_id": "CAB-D",
                "current_roe_id": "ROE-OLD",
                "candidate_roe_id": "ROE-NEW",
                "inventory_fingerprint": "a" * 64,
                "reason_code": "TEST_DECISION",
                "risk": "LOW",
            }
            result = evaluate_worker(
                {
                    "agent_id": "AGENT-CAB-D",
                    "cabinet_id": "CAB-D",
                    "context": context,
                }
            )
            runtime.roe_selection_decision.assert_called_once_with(
                "AGENT-CAB-D",
                "CAB-D",
                context,
            )
            if result.get("decision", {}).get("decision") != decision:
                return False
    return True


def test_invalid_codex_response_fails_closed() -> bool:
    context = {
        "candidate_cabinet_id": "CAB-D",
        "current_roe_id": None,
        "candidate_roe_id": "ROE-NEW",
        "candidate_splitter_id": "SPL-D-01",
        "distance_m": 32.5,
        "available_fibers": 4,
        "required_fibers": 1,
        "inventory_fingerprint": "a" * 64,
    }
    completed = subprocess.CompletedProcess(
        args=["codex"],
        returncode=0,
        stdout='{"decision":[],"risk":{},"agent_id":"AGENT-CAB-D"}',
        stderr="",
    )
    with patch("agent.llm_runtime.subprocess.run", return_value=completed):
        result = LLMRuntime(codex_path="mock-codex").roe_selection_decision(
            "AGENT-CAB-D",
            "CAB-D",
            context,
        )
    return result.get("success") is False


def test_photo_worker_uses_actual_temporary_image_with_mock_provider() -> bool:
    with tempfile.NamedTemporaryFile(prefix="fiberspider-", suffix=".jpg") as image:
        image.write(b"synthetic-image-fixture")
        image.flush()
        with patch("agent.decision_worker.LLMRuntime") as runtime_type:
            runtime = runtime_type.return_value
            runtime.extract_ont_label.return_value = {
                "success": True,
                "serial": "ONT-EXAMPLE",
                "gpon_port_id": "PORT-EXAMPLE",
                "confidence": 0.95,
            }
            result = evaluate_worker({
                "operation": "EXTRACT_ONT_LABEL",
                "agent_id": "AGENT-CAB-D",
                "cabinet_id": "CAB-D",
                "context": {"cabinet_id": "CAB-D", "image_path": image.name},
            })
            runtime.extract_ont_label.assert_called_once_with("AGENT-CAB-D", "CAB-D", image.name)
            return result.get("success") is True and result["ont_label"]["serial"] == "ONT-EXAMPLE"


if __name__ == "__main__":
    result = test_codex_stderr_diagnostics_no_command_detection()
    allow = test_roe_decision_allow_is_structured_and_mocked()
    deny = test_roe_decision_deny_is_structured_and_mocked()
    wrong_cabinet = test_roe_decision_wrong_cabinet_fails_closed()
    worker = test_worker_routes_allow_and_deny_to_the_selected_cabinet_agent()
    invalid = test_invalid_codex_response_fails_closed()
    photo = test_photo_worker_uses_actual_temporary_image_with_mock_provider()
    success = result and allow and deny and wrong_cabinet and worker and invalid and photo
    print("TEST PASS" if success else "TEST FAIL")
    raise SystemExit(0 if success else 1)
