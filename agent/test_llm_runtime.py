from __future__ import annotations

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


if __name__ == "__main__":
    result = test_codex_stderr_diagnostics_no_command_detection()
    print("TEST PASS" if result else "TEST FAIL")
    raise SystemExit(0 if result else 1)
