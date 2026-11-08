from __future__ import annotations

import json
import os
import shutil
import sys
import tempfile
from pathlib import Path
from typing import Any

from .cabinet_agent import CabinetAgent
from .llm_runtime import LLMRuntime


def evaluate(request: dict[str, Any]) -> dict[str, Any]:
    if set(request) not in (
        {"agent_id", "cabinet_id", "context"},
        {"operation", "agent_id", "cabinet_id", "context"},
    ):
        return {"success": False, "error": "invalid worker request"}
    agent_id = request.get("agent_id")
    cabinet_id = request.get("cabinet_id")
    context = request.get("context")
    if (
        not isinstance(agent_id, str)
        or not isinstance(cabinet_id, str)
        or not isinstance(context, dict)
    ):
        return {"success": False, "error": "invalid worker request"}
    operation = request.get("operation", "ROE_SELECTION_DECISION")
    codex_path = (
        os.environ.get("FIBERSPIDER_CODEX_PATH")
        or shutil.which("codex")
        or "/usr/local/bin/codex"
    )
    agent = CabinetAgent(
        agent_id=agent_id,
        cabinet_id=cabinet_id,
        llm_runtime=LLMRuntime(codex_path=codex_path),
    )
    if operation == "EXTRACT_ONT_LABEL":
        image_path = context.get("image_path")
        if not isinstance(image_path, str):
            return {"success": False, "error": "invalid image path"}
        image = Path(image_path)
        temporary_root = Path(tempfile.gettempdir()).resolve()
        if (
            image.is_symlink()
            or not image.is_file()
            or image.resolve().parent != temporary_root
            or not image.name.startswith("fiberspider-")
        ):
            return {"success": False, "error": "invalid image path"}
        result = agent.extract_ont_label(image_path, context)
        if result.get("success") is not True:
            return result
        return {
            "success": True,
            "ont_label": {
                "serial": result["serial"],
                "gpon_port_id": result["gpon_port_id"],
                "confidence": result["confidence"],
            },
            "error": None,
        }
    if operation == "ONT_PORT_CONFLICT":
        result = agent.analyze_ont_port_conflict(context)
        if result.get("success") is not True:
            return result
        fields = {
            "decision",
            "agent_id",
            "cabinet_id",
            "gpon_port_id",
            "reason_code",
            "alert",
        }
        if not fields.issubset(result):
            return {"success": False, "error": "invalid port conflict result"}
        return {
            "success": True,
            "port_conflict": {key: result[key] for key in fields},
            "error": None,
        }
    if operation != "ROE_SELECTION_DECISION":
        return {"success": False, "error": "unsupported worker operation"}
    result = agent.evaluate_roe_selection(context)
    if result.get("success") is not True:
        return result
    decision_fields = {
        "decision",
        "agent_id",
        "cabinet_id",
        "current_roe_id",
        "candidate_roe_id",
        "inventory_fingerprint",
        "reason_code",
        "risk",
    }
    if not decision_fields.issubset(result):
        return {"success": False, "error": "invalid structured decision"}
    return {
        "success": True,
        "decision": {key: result[key] for key in decision_fields},
        "error": None,
    }


def main() -> int:
    try:
        request = json.load(sys.stdin)
    except (json.JSONDecodeError, UnicodeDecodeError):
        response = {"success": False, "error": "invalid worker request"}
    else:
        response = evaluate(request) if isinstance(request, dict) else {
            "success": False,
            "error": "invalid worker request",
        }
    sys.stdout.write(json.dumps(response, separators=(",", ":")))
    sys.stdout.write("\n")
    return 0 if response.get("success") is True else 1


if __name__ == "__main__":
    raise SystemExit(main())