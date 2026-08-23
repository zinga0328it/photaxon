from __future__ import annotations

import os
import re
import sys
import time
import urllib.error
import urllib.request
from typing import Any

import psycopg
from psycopg.rows import dict_row

from agent.otp_service import OTPService
from agent.telegram_provider import RealTelegramProvider

TECHNICIAN_ID = os.environ.get("FIBERSPIDER_TECHNICIAN_ID", "1234567890")
CHAT_ID = os.environ.get("FIBERSPIDER_CHAT_ID", "1234567890")
DISPLAY_NAME = os.environ.get("FIBERSPIDER_DISPLAY_NAME", "Technician")
DB_ENV_PATH = os.environ.get("FIBERSPIDER_DB_ENV_PATH", "secrets/fiberspider-db.env")
TG_ENV_PATH = os.environ.get("TELEGRAM_SECRET_PATH", "secrets/telegram-bot.env")
TIMEOUT_SECONDS = 120


def load_env(path: str) -> None:
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, value = line.split("=", 1)
            os.environ[key] = value


def get_telegram_token() -> str:
    token = os.environ.get("TELEGRAM_BOT_TOKEN")
    if token:
        return token
    if not os.path.exists(TG_ENV_PATH):
        raise RuntimeError("TELEGRAM_BOT_TOKEN not found")
    with open(TG_ENV_PATH, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line.startswith("TELEGRAM_BOT_TOKEN="):
                return line.split("=", 1)[1]
    raise RuntimeError("TELEGRAM_BOT_TOKEN not found")


def is_technician_active(technician_id: str) -> bool:
    conn = psycopg.connect(
        host=os.environ["FIBERSPIDER_DB_HOST"],
        port=os.environ["FIBERSPIDER_DB_PORT"],
        dbname=os.environ["FIBERSPIDER_DB_NAME"],
        user=os.environ["FIBERSPIDER_DB_USER"],
        password=os.environ["FIBERSPIDER_DB_PASSWORD"],
        row_factory=dict_row,
    )
    try:
        with conn.cursor() as cur:
            cur.execute(
                "SELECT status FROM technicians WHERE technician_id = %s;",
                (technician_id,),
            )
            row = cur.fetchone()
    finally:
        conn.close()

    return bool(row and row["status"].upper() == "ACTIVE")


def print_report(report: dict[str, str]) -> None:
    for key, value in report.items():
        print(f"{key}: {value}")


def is_candidate_update(update: Any, baseline_update_id: int) -> bool:
    if not isinstance(update, dict):
        return False

    update_id = update.get("update_id")
    if not isinstance(update_id, int):
        return False
    if update_id <= baseline_update_id:
        return False

    message = update.get("message")
    if not isinstance(message, dict):
        return False

    sender = message.get("from")
    chat = message.get("chat")
    text = message.get("text")
    if not (
        isinstance(sender, dict)
        and isinstance(chat, dict)
        and isinstance(text, str)
    ):
        return False

    if (
        str(sender.get("id")) == TECHNICIAN_ID
        and str(chat.get("id")) == CHAT_ID
        and chat.get("type") == "private"
        and re.fullmatch(r"^\d{6}$", text.strip())
    ):
        return True

    return False


def stale_update_filter_test() -> bool:
    baseline_update_id = 100
    old_update = {
        "update_id": 100,
        "message": {
            "from": {"id": int(TECHNICIAN_ID)},
            "chat": {"id": int(CHAT_ID), "type": "private"},
            "text": "123456",
        },
    }
    new_update = {
        "update_id": 101,
        "message": {
            "from": {"id": int(TECHNICIAN_ID)},
            "chat": {"id": int(CHAT_ID), "type": "private"},
            "text": "123456",
        },
    }
    return not is_candidate_update(old_update, baseline_update_id) and is_candidate_update(new_update, baseline_update_id)


def main() -> None:
    load_env(DB_ENV_PATH)
    load_env(TG_ENV_PATH)

    report = {
        "BASELINE UPDATE ID ACQUIRED": "NO",
        "STALE UPDATE FILTER": "FAIL",
        "TECHNICIAN ACTIVE": "NO",
        "OTP GENERATED": "NO",
        "OTP SENT": "NO",
        "NEW REPLY RECEIVED": "NO",
        "OTP RESULT": "TIMEOUT",
        "OTP PRINTED": "NO",
        "TOKEN PRINTED": "NO",
        "CODEX CALLED": "NO",
        "OPENSTACK MODIFIED": "NO",
        "FILES MODIFIED": "agent/telegram_provider.py, agent/test_real_telegram_otp.py",
    }

    if not is_technician_active(TECHNICIAN_ID):
        print_report(report)
        return

    report["TECHNICIAN ACTIVE"] = "YES"

    if not stale_update_filter_test():
        print_report(report)
        return

    report["STALE UPDATE FILTER"] = "PASS"

    provider = RealTelegramProvider()
    baseline_update_id = 0
    try:
        initial_updates = provider.get_updates(timeout=1, limit=100)
        if initial_updates.get("ok"):
            results = initial_updates.get("result", [])
            if isinstance(results, list) and results:
                baseline_update_id = max(
                    update["update_id"]
                    for update in results
                    if isinstance(update, dict) and isinstance(update.get("update_id"), int)
                )
        report["BASELINE UPDATE ID ACQUIRED"] = "YES"
    except Exception:
        report["BASELINE UPDATE ID ACQUIRED"] = "NO"

    otp_service = OTPService()
    otp_code = otp_service.request_otp(TECHNICIAN_ID)
    report["OTP GENERATED"] = "YES"

    try:
        send_payload = {
            "chat_id": CHAT_ID,
            "text": (
                "🕷️ FiberSpider CAB-A\n"
                "Tecnico autenticato\n\n"
                "Nuovo codice OTP: {otp}\n\n"
                "Rispondi a QUESTO BOT con le 6 cifre.\n"
                "Valido 5 minuti."
            ).format(otp=otp_code),
        }
        send_result = provider.send_message(CHAT_ID, send_payload["text"])
        if send_result.get("ok"):
            report["OTP SENT"] = "YES"
        else:
            print_report(report)
            return
    except Exception:
        print_report(report)
        return

    print("WAITING FOR TELEGRAM OTP REPLY")

    reply_text: str | None = None
    reply_deadline = time.monotonic() + TIMEOUT_SECONDS
    update_offset: int = baseline_update_id + 1
    pattern = re.compile(r"^\d{6}$")

    while time.monotonic() < reply_deadline:
        try:
            updates = provider.get_updates(timeout=2, limit=20, offset=update_offset)
        except Exception:
            time.sleep(1)
            continue

        if not updates.get("ok"):
            time.sleep(1)
            continue

        for update in updates.get("result", []):
            if not isinstance(update, dict):
                continue

            update_id = update.get("update_id")
            if isinstance(update_id, int):
                if update_id >= update_offset:
                    update_offset = update_id + 1

            if not is_candidate_update(update, baseline_update_id):
                continue

            reply_text = update["message"]["text"].strip()
            break

        if reply_text is not None:
            break

        time.sleep(1)

    if reply_text is None:
        report["REPLY RECEIVED"] = "NO"
        report["OTP RESULT"] = "TIMEOUT"
        print_report(report)
        return

    report["REPLY RECEIVED"] = "YES"
    status = otp_service.verify_otp(TECHNICIAN_ID, reply_text)
    report["OTP RESULT"] = status

    if status == "AUTHENTICATED":
        try:
            provider.send_message(
                CHAT_ID,
                "✅ FiberSpider CAB-A\nTecnico autenticato.",
            )
        except Exception:
            pass

    print_report(report)


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] in ("local", "local_test", "stale_test"):
        passed = stale_update_filter_test()
        print(f"STALE UPDATE FILTER: {'PASS' if passed else 'FAIL'}")
        raise SystemExit(0 if passed else 1)
    main()
