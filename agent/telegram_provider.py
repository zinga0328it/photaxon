from __future__ import annotations

import json
import os
import urllib.parse
import urllib.request
from abc import ABC, abstractmethod
from typing import Any


class TelegramProvider(ABC):
    """Abstract Telegram provider interface for future bot integration."""

    @abstractmethod
    def get_bot_identity(self) -> dict[str, Any]:
        raise NotImplementedError

    @abstractmethod
    def find_chat_for_user(self, telegram_user_id: str) -> str | None:
        raise NotImplementedError

    @abstractmethod
    def send_message(self, chat_id: str, text: str) -> dict[str, Any]:
        raise NotImplementedError

    @abstractmethod
    def send_otp(self, telegram_chat_id: str, otp: str) -> dict[str, Any]:
        raise NotImplementedError

    @abstractmethod
    def get_updates(self, timeout: int = 0, limit: int = 20, offset: int | None = None) -> dict[str, Any]:
        raise NotImplementedError


class MockTelegramProvider(TelegramProvider):
    """Mock Telegram provider that records OTP delivery without network."""

    def __init__(self) -> None:
        self.sent_messages: list[tuple[str, str]] = []

    def get_bot_identity(self) -> dict[str, Any]:
        return {"ok": True, "result": {"username": "mockbot"}}

    def find_chat_for_user(self, telegram_user_id: str) -> str | None:
        return None

    def send_message(self, chat_id: str, text: str) -> dict[str, Any]:
        self.sent_messages.append((chat_id, text))
        return {"ok": True, "result": {"chat": {"id": chat_id}, "text": text}}

    def send_otp(self, telegram_chat_id: str, otp: str) -> dict[str, Any]:
        text = f"FiberSpider OTP: {otp}"
        return self.send_message(telegram_chat_id, text)

    def get_updates(self, timeout: int = 0, limit: int = 20, offset: int | None = None) -> dict[str, Any]:
        payload: dict[str, Any] = {"timeout": timeout, "limit": limit}
        if offset is not None:
            payload["offset"] = offset
        return self._call("getUpdates", payload)


class RealTelegramProvider(TelegramProvider):
    """Real Telegram provider using HTTPS and environment token."""

    def __init__(self, timeout: int = 10) -> None:
        self.timeout = timeout
        self.token = os.environ.get("TELEGRAM_BOT_TOKEN")
        if not self.token:
            raise RuntimeError("TELEGRAM_BOT_TOKEN is required in environment")

    def _api_url(self, method: str) -> str:
        return f"https://api.telegram.org/bot{self.token}/{method}"

    def _call(self, method: str, data: dict[str, Any] | None = None) -> dict[str, Any]:
        url = self._api_url(method)
        headers = {"Content-Type": "application/json"}
        body = json.dumps(data).encode("utf-8") if data is not None else None
        request = urllib.request.Request(url, data=body, headers=headers)
        with urllib.request.urlopen(request, timeout=self.timeout) as response:
            response_text = response.read().decode("utf-8")
        return json.loads(response_text)

    def get_bot_identity(self) -> dict[str, Any]:
        return self._call("getMe")

    def find_chat_for_user(self, telegram_user_id: str) -> str | None:
        updates = self._call("getUpdates", {"timeout": 0, "limit": 100})
        if not updates.get("ok"):
            return None
        for update in updates.get("result", []):
            message = update.get("message") or update.get("edited_message") or update.get("channel_post") or update.get("edited_channel_post")
            if message:
                sender = message.get("from")
                chat = message.get("chat")
                if sender and chat and str(sender.get("id")) == str(telegram_user_id):
                    return str(chat.get("id"))
            callback = update.get("callback_query")
            if callback:
                sender = callback.get("from")
                message = callback.get("message")
                if sender and message and str(sender.get("id")) == str(telegram_user_id):
                    chat = message.get("chat")
                    if chat:
                        return str(chat.get("id"))
        return None

    def send_message(self, chat_id: str, text: str) -> dict[str, Any]:
        return self._call("sendMessage", {"chat_id": str(chat_id), "text": text})

    def send_otp(self, telegram_chat_id: str, otp: str) -> dict[str, Any]:
        text = f"FiberSpider OTP: {otp}"
        return self.send_message(str(telegram_chat_id), text)

    def get_updates(self, timeout: int = 0, limit: int = 20, offset: int | None = None) -> dict[str, Any]:
        payload: dict[str, Any] = {"timeout": timeout, "limit": limit}
        if offset is not None:
            payload["offset"] = offset
        return self._call("getUpdates", payload)
