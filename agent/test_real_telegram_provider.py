from __future__ import annotations

import os

from agent.telegram_provider import RealTelegramProvider


def load_token_from_secret_file() -> bool:
    secret_path = os.environ.get("TELEGRAM_SECRET_PATH", "secrets/telegram-bot.env")
    if not os.path.exists(secret_path):
        return False
    with open(secret_path, "r", encoding="utf-8") as secret_file:
        for line in secret_file:
            line = line.strip()
            if line.startswith("TELEGRAM_BOT_TOKEN="):
                token = line.split("=", 1)[1]
                os.environ["TELEGRAM_BOT_TOKEN"] = token
                return True
    return False


def run_test() -> dict[str, str | bool]:
    if not load_token_from_secret_file():
        return {
            "bot_connected": False,
            "bot_username": "",
            "telegram_user_found": False,
            "chat_id_found": False,
            "test_message_sent": False,
        }

    provider = RealTelegramProvider()
    bot_identity = provider.get_bot_identity()
    connected = bot_identity.get("ok") is True
    username = bot_identity.get("result", {}).get("username", "")

    if not connected:
        return {
            "bot_connected": False,
            "bot_username": username,
            "telegram_user_found": False,
            "chat_id_found": False,
            "test_message_sent": False,
        }

    user_id = os.environ.get("FIBERSPIDER_TECHNICIAN_ID", "1234567890")
    chat_id = provider.find_chat_for_user(user_id)
    if not chat_id:
        return {
            "bot_connected": True,
            "bot_username": username,
            "telegram_user_found": False,
            "chat_id_found": False,
            "test_message_sent": False,
        }

    message = "🕷️ FiberSpider CAB-A collegato. Tecnico autenticato riconosciuto."
    send_result = provider.send_message(chat_id, message)
    sent = send_result.get("ok") is True
    return {
        "bot_connected": True,
        "bot_username": username,
        "telegram_user_found": True,
        "chat_id_found": bool(chat_id),
        "test_message_sent": sent,
    }


if __name__ == "__main__":
    result = run_test()
    print(result)
    raise SystemExit(0 if result["bot_connected"] else 1)
