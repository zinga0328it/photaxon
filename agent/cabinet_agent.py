from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from engine.postgres_store import PostgresStore
from .llm_runtime import LLMRuntime
from .otp_provider import OTPProvider, MockOTPProvider
from .otp_service import OTPService
from .telegram_provider import TelegramProvider, MockTelegramProvider


@dataclass
class Event:
    type: str
    payload: dict[str, Any]


class CabinetAgent:
    """Minimal micro-agent for a FiberSpider cabinet."""

    def __init__(
        self,
        agent_id: str,
        cabinet_id: str,
        llm_runtime: LLMRuntime | None = None,
        store: PostgresStore | None = None,
        otp_provider: OTPProvider | None = None,
        otp_service: OTPService | None = None,
        telegram_provider: TelegramProvider | None = None,
    ) -> None:
        self.agent_id = agent_id
        self.cabinet_id = cabinet_id
        self.llm_runtime = llm_runtime or LLMRuntime()
        self.store = store or PostgresStore()
        self.otp_provider = otp_provider or MockOTPProvider()
        self.otp_service = otp_service or OTPService()
        self.telegram_provider = telegram_provider or MockTelegramProvider()

    def handle_event(self, event: Event, dry_run: bool = True) -> dict[str, Any]:
        """Handle an event from the cabinet environment."""
        if event.type == "TECHNICIAN_ARRIVED":
            return self._handle_technician_arrived(event, dry_run=dry_run)
        if event.type == "TECHNICIAN_OTP_SUBMITTED":
            return self._handle_technician_otp_submitted(event)
        return {
            "agent_id": self.agent_id,
            "cabinet_id": self.cabinet_id,
            "event": event.type,
            "action": "noop",
            "proposal": None,
        }

    def _handle_technician_arrived(self, event: Event, dry_run: bool = True) -> dict[str, Any]:
        technician_id = event.payload.get("technician_id")
        if not technician_id:
            return {
                "agent_id": self.agent_id,
                "cabinet_id": self.cabinet_id,
                "event": event.type,
                "action": "auth_required",
                "proposal": {
                    "decision": "AUTENTICAZIONE_RICHIESTA",
                    "next_action": "INSERIRE ID TECNICO",
                    "reason": "Nessun technician_id fornito.",
                },
            }

        if dry_run:
            return {
                "agent_id": self.agent_id,
                "cabinet_id": self.cabinet_id,
                "event": event.type,
                "action": "dry_run",
                "proposal": {
                    "decision": "OTP_REQUIRED",
                    "next_action": "RICHIEDERE OTP DA TELEGRAM",
                    "reason": "Simulazione richiesta OTP dopo ID valido.",
                },
            }

        self.store.connect()
        authorized = self.store.is_technician_authorized(str(technician_id))
        if not authorized:
            return {
                "agent_id": self.agent_id,
                "cabinet_id": self.cabinet_id,
                "event": event.type,
                "action": "access_denied",
                "proposal": {
                    "decision": "ACCESS_DENIED",
                    "next_action": "RICHIEDERE TECNICO AUTORIZZATO",
                    "reason": f"Tecnico {technician_id} non trovato o non autorizzato.",
                },
            }

        otp_code = self.otp_service.request_otp(str(technician_id))
        telegram_chat_id = event.payload.get("telegram_chat_id")
        if not telegram_chat_id:
            telegram_chat_id = self.telegram_provider.find_chat_for_user(str(technician_id))
        if telegram_chat_id:
            self.telegram_provider.send_otp(str(telegram_chat_id), otp_code)

        return {
            "agent_id": self.agent_id,
            "cabinet_id": self.cabinet_id,
            "event": event.type,
            "action": "otp_required",
            "proposal": {
                "decision": "OTP_REQUIRED",
                "next_action": "INVIARE OTP SU TELEGRAM",
                "reason": f"Tecnico {technician_id} autorizzato. OTP richiesto per verifica.",
            },
        }

    def _handle_technician_otp_submitted(self, event: Event) -> dict[str, Any]:
        technician_id = event.payload.get("technician_id")
        otp = event.payload.get("otp")
        if not technician_id or not otp:
            return {
                "agent_id": self.agent_id,
                "cabinet_id": self.cabinet_id,
                "event": event.type,
                "action": "otp_invalid",
                "proposal": {
                    "decision": "OTP_INVALID",
                    "next_action": "INSERIRE OTP VALIDO",
                    "reason": "Technician ID o OTP mancante.",
                },
            }

        status = self.otp_service.verify_otp(str(technician_id), str(otp))
        if status == "AUTHENTICATED":
            return {
                "agent_id": self.agent_id,
                "cabinet_id": self.cabinet_id,
                "event": event.type,
                "action": "authenticated",
                "proposal": {
                    "decision": "AUTHENTICATED",
                    "next_action": "PROSEGUIRE CON IL FLUSSO FTTH",
                    "reason": "OTP verificato correttamente.",
                },
            }
        if status == "OTP_EXPIRED":
            return {
                "agent_id": self.agent_id,
                "cabinet_id": self.cabinet_id,
                "event": event.type,
                "action": "otp_expired",
                "proposal": {
                    "decision": "OTP_EXPIRED",
                    "next_action": "RICHIEDERE NUOVO OTP",
                    "reason": "OTP scaduto.",
                },
            }
        if status == "OTP_LOCKED":
            return {
                "agent_id": self.agent_id,
                "cabinet_id": self.cabinet_id,
                "event": event.type,
                "action": "otp_locked",
                "proposal": {
                    "decision": "OTP_LOCKED",
                    "next_action": "RICHIEDERE SUPPORTO",
                    "reason": "Troppi tentativi errati. OTP bloccato.",
                },
            }
        return {
            "agent_id": self.agent_id,
            "cabinet_id": self.cabinet_id,
            "event": event.type,
            "action": "otp_invalid",
            "proposal": {
                "decision": "OTP_INVALID",
                "next_action": "RIPETERE OTP",
                "reason": "OTP errato.",
            },
        }
