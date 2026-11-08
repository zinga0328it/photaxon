from __future__ import annotations

import json
import subprocess
from typing import Any


class LLMRuntime:
    """Adapter for the OpenAI Codex CLI runtime using stdin-only prompt transport."""

    def __init__(self, codex_path: str = "/usr/local/bin/codex") -> None:
        self.codex_path = codex_path

    def _sanitize_output(self, output: str) -> str:
        return output.strip()

    def _is_normal_codex_stderr_line(self, line: str) -> bool:
        lower = line.lower().strip()
        normal_prefixes = [
            "reading prompt from stdin",
            "openai codex",
            "--------",
            "workdir:",
            "model:",
            "provider:",
            "approval:",
            "sandbox:",
            "reasoning effort:",
            "reasoning summaries:",
            "session id:",
            "tokens used",
            "warning: codex's linux sandbox",
            "user",
        ]
        return any(lower.startswith(prefix) for prefix in normal_prefixes)

    def _contains_disallowed_terms(self, text: str, ignore_codex_diagnostics: bool = False) -> bool:
        forbidden = [
            "ssh",
            "openstack",
            "postgres",
            "psql",
            "nftables",
            "sudo",
            "bash",
            "shell",
            "curl",
            "wget",
        ]

        if not ignore_codex_diagnostics:
            lower = text.lower()
            return any(term in lower for term in forbidden)

        in_user_block = False
        for line in text.splitlines():
            stripped = line.strip()
            if not stripped:
                continue
            lower = stripped.lower()
            if lower == "user":
                in_user_block = True
                continue
            if in_user_block:
                continue
            if self._is_normal_codex_stderr_line(stripped):
                continue
            if any(term in lower for term in forbidden):
                return True
        return False

    def reason(self, agent_id: str, event: str, context: dict[str, Any]) -> dict[str, Any]:
        """Generate a safe advisory proposal from the Codex CLI.

        Uses `/usr/local/bin/codex exec` with:
        - --ephemeral
        - --sandbox read-only
        - stdin for prompt transport

        The response must contain only the requested short fields.
        """
        prompt_text = (
            "Sei l'agente consultivo dell'armadio FTTH CAB-A.\n"
            "NON eseguire comandi.\n"
            "NON leggere filesystem.\n"
            "NON modificare rete.\n"
            "NON modificare database.\n"
            "NON modificare OpenStack.\n"
            "Devi soltanto valutare l'evento ricevuto.\n\n"
            f"EVENTO:\n{event}\n\n"
            "CONTESTO:\n"
            f"request_id={context.get('request_id', '')}\n"
            f"technician_id={context.get('technician_id', '')}\n"
            f"cabinet_id={context.get('cabinet_id', '')}\n"
            "nessun ONT/modem ancora rilevato.\n"
            "nessuna porta ancora scelta.\n"
            "nessun provisioning ancora autorizzato.\n\n"
            "Per TECHNICIAN_ARRIVED, se mancano identificazione apparato e porta fisica, NON proporre provisioning.\n\n"
            "Rispondi brevissimamente con SOLO:\n"
            "DECISION:\n"
            "NEXT_ACTION:\n"
            "REASON:\n"
        )

        command = [
            self.codex_path,
            "exec",
            "--ephemeral",
            "--sandbox",
            "read-only",
        ]

        result = subprocess.run(
            command,
            input=prompt_text,
            capture_output=True,
            text=True,
            check=False,
            timeout=45,
        )

        stdout = self._sanitize_output(result.stdout)
        stderr = self._sanitize_output(result.stderr)

        fields: dict[str, str] = {
            "DECISION": "",
            "NEXT_ACTION": "",
            "REASON": "",
        }

        for line in stdout.splitlines():
            if ":" not in line:
                continue
            key, value = line.split(":", 1)
            normalized = key.strip().upper()
            if normalized in fields:
                fields[normalized] = value.strip()

        fields_present = all(fields.values())
        banned = self._contains_disallowed_terms(stdout) or self._contains_disallowed_terms(stderr, ignore_codex_diagnostics=True)

        return {
            "returncode": result.returncode,
            "stdout": stdout,
            "stderr": stderr,
            "fields": fields,
            "success": fields_present and result.returncode == 0 and not banned,
            "commands_detected": banned,
        }

    def roe_selection_decision(
        self,
        agent_id: str,
        cabinet_id: str,
        context: dict[str, Any],
    ) -> dict[str, Any]:
        required_fields = {
            "current_roe_id",
            "candidate_roe_id",
            "candidate_splitter_id",
            "distance_m",
            "available_fibers",
            "required_fibers",
            "inventory_fingerprint",
        }
        if not required_fields.issubset(context):
            return {"success": False, "error": "invalid decision input"}
        if context.get("candidate_cabinet_id") != cabinet_id:
            return {"success": False, "error": "cabinet identity mismatch"}

        request = {
            "agent_id": agent_id,
            "cabinet_id": cabinet_id,
            "current_roe_id": context["current_roe_id"],
            "candidate_roe_id": context["candidate_roe_id"],
            "candidate_splitter_id": context["candidate_splitter_id"],
            "distance_m": context["distance_m"],
            "available_fibers": context["available_fibers"],
            "required_fibers": context["required_fibers"],
            "inventory_fingerprint": context["inventory_fingerprint"],
        }
        prompt = (
            "Sei il cabinet agent selezionato. Fornisci solo una decisione consultiva "
            "sulla proposta ROE ricevuta. Non eseguire azioni, non usare tool, non "
            "scrivere dati e non autorizzare capacità negativa o cabinet diverso. "
            "Restituisci esclusivamente un oggetto JSON con le chiavi: decision "
            "(ALLOW, ALLOW_WITH_WARNING, DENY, NEEDS_SUPERVISOR), agent_id, cabinet_id, "
            "current_roe_id, candidate_roe_id, inventory_fingerprint, reason_code "
            "(A-Z, cifre e underscore), risk (LOW, MEDIUM, HIGH). Copia senza "
            "alterazioni identità e fingerprint dall'input.\nINPUT_JSON:\n"
            f"{json.dumps(request, separators=(',', ':'), sort_keys=True)}"
        )
        try:
            result = subprocess.run(
                [self.codex_path, "exec", "--ephemeral", "--sandbox", "read-only"],
                input=prompt,
                capture_output=True,
                text=True,
                check=False,
                timeout=45,
            )
        except (OSError, subprocess.TimeoutExpired):
            return {"success": False, "error": "cabinet decision provider unavailable"}
        if result.returncode != 0:
            return {"success": False, "error": "cabinet decision provider failed"}
        try:
            response = json.loads(result.stdout)
        except (TypeError, json.JSONDecodeError):
            return {"success": False, "error": "invalid structured decision"}
        if not isinstance(response, dict):
            return {"success": False, "error": "invalid structured decision"}
        expected_keys = {
            "decision",
            "agent_id",
            "cabinet_id",
            "current_roe_id",
            "candidate_roe_id",
            "inventory_fingerprint",
            "reason_code",
            "risk",
        }
        if set(response) != expected_keys:
            return {"success": False, "error": "invalid decision fields"}
        if not isinstance(response["decision"], str) or response["decision"] not in {
            "ALLOW",
            "ALLOW_WITH_WARNING",
            "DENY",
            "NEEDS_SUPERVISOR",
        }:
            return {"success": False, "error": "invalid decision value"}
        if not isinstance(response["risk"], str) or response["risk"] not in {
            "LOW",
            "MEDIUM",
            "HIGH",
        }:
            return {"success": False, "error": "invalid risk value"}
        for key in ("agent_id", "cabinet_id", "candidate_roe_id", "inventory_fingerprint"):
            if response[key] != request[key]:
                return {"success": False, "error": "decision identity mismatch"}
        if response["current_roe_id"] != request["current_roe_id"]:
            return {"success": False, "error": "decision identity mismatch"}
        reason_code = response["reason_code"]
        if (
            not isinstance(reason_code, str)
            or not reason_code
            or len(reason_code) > 64
            or not all(char.isupper() or char.isdigit() or char == "_" for char in reason_code)
        ):
            return {"success": False, "error": "invalid reason code"}
        return {"success": True, **response}

    def extract_ont_label(
        self,
        agent_id: str,
        cabinet_id: str,
        image_path: str,
    ) -> dict[str, Any]:
        prompt = (
            "Sei il cabinet agent selezionato. Leggi esclusivamente l'etichetta "
            "della confezione ONT nell'immagine allegata. Estrai il seriale ONT e "
            "l'identificativo GPON/porta se stampato. Non inventare valori: usa una "
            "stringa vuota per i campi non leggibili e confidence tra 0 e 1. "
            "Rispondi con SOLO JSON: {\"serial\":string,\"gpon_port_id\":string,"
            "\"confidence\":number}. Non chiamare strumenti, non accedere a DB e "
            "non eseguire azioni.\n"
            f"CABINET_AGENT={agent_id}\nCABINET_ID={cabinet_id}"
        )
        try:
            result = subprocess.run(
                [
                    self.codex_path,
                    "exec",
                    "--ephemeral",
                    "--sandbox",
                    "read-only",
                    "--image",
                    image_path,
                ],
                input=prompt,
                capture_output=True,
                text=True,
                check=False,
                timeout=45,
            )
        except (OSError, subprocess.TimeoutExpired):
            return {"success": False, "error": "ONT label provider unavailable"}
        if result.returncode != 0:
            return {"success": False, "error": "ONT label provider failed"}
        try:
            response = json.loads(result.stdout)
        except (TypeError, json.JSONDecodeError):
            return {"success": False, "error": "ONT label response is not JSON"}
        if not isinstance(response, dict) or set(response) != {
            "serial",
            "gpon_port_id",
            "confidence",
        }:
            return {"success": False, "error": "ONT label response has invalid fields"}
        if (
            not isinstance(response["serial"], str)
            or not isinstance(response["gpon_port_id"], str)
            or not isinstance(response["confidence"], (int, float))
            or isinstance(response["confidence"], bool)
            or not 0.0 <= float(response["confidence"]) <= 1.0
        ):
            return {"success": False, "error": "ONT label response has invalid values"}
        return {"success": True, **response}

    def analyze_ont_port_conflict(
        self,
        agent_id: str,
        cabinet_id: str,
        context: dict[str, Any],
    ) -> dict[str, Any]:
        required = {
            "gpon_port_id",
            "resource_state",
            "active_ont_assigned",
            "customer_assignment_present",
            "requested_serial",
            "work_order_id",
        }
        if not required.issubset(context) or context.get("cabinet_id") != cabinet_id:
            return {"success": False, "error": "invalid port conflict context"}
        request = {
            "agent_id": agent_id,
            "cabinet_id": cabinet_id,
            **{key: context[key] for key in required},
        }
        prompt = (
            "Sei il cabinet agent del cabinet selezionato. Analizza il conflitto "
            "GPON leggendo solo il contesto fornito. Se la porta non è FREE, è "
            "assegnata a un'ONT o a un cliente, il provisioning richiesto è un "
            "errore: rispondi DENY e genera un alert breve al tecnico. Non puoi "
            "autorizzare né modificare assegnazioni. Restituisci solo JSON con "
            "decision=DENY, agent_id, cabinet_id, gpon_port_id, reason_code "
            "(A-Z/digit/underscore), alert (massimo 240 caratteri).\nINPUT_JSON:\n"
            f"{json.dumps(request, separators=(',', ':'), sort_keys=True)}"
        )
        try:
            result = subprocess.run(
                [self.codex_path, "exec", "--ephemeral", "--sandbox", "read-only"],
                input=prompt,
                capture_output=True,
                text=True,
                check=False,
                timeout=45,
            )
        except (OSError, subprocess.TimeoutExpired):
            return {"success": False, "error": "cabinet agent unavailable"}
        if result.returncode != 0:
            return {"success": False, "error": "cabinet agent failed"}
        try:
            response = json.loads(result.stdout)
        except (TypeError, json.JSONDecodeError):
            return {"success": False, "error": "invalid port conflict response"}
        expected = {"decision", "agent_id", "cabinet_id", "gpon_port_id", "reason_code", "alert"}
        if not isinstance(response, dict) or set(response) != expected:
            return {"success": False, "error": "invalid port conflict fields"}
        if (
            response["decision"] != "DENY"
            or response["agent_id"] != agent_id
            or response["cabinet_id"] != cabinet_id
            or response["gpon_port_id"] != context["gpon_port_id"]
            or not isinstance(response["reason_code"], str)
            or not response["reason_code"]
            or len(response["reason_code"]) > 64
            or not all(
                char.isupper() or char.isdigit() or char == "_"
                for char in response["reason_code"]
            )
            or not isinstance(response["alert"], str)
            or not response["alert"].strip()
            or len(response["alert"]) > 240
        ):
            return {"success": False, "error": "invalid port conflict decision"}
        return {"success": True, **response}
