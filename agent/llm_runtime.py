from __future__ import annotations

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
