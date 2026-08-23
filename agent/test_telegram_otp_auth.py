from __future__ import annotations

from agent.cabinet_agent import CabinetAgent, Event
from agent.otp_provider import MockOTPProvider
from engine.postgres_store import PostgresStore


class MockPostgresStore(PostgresStore):
    def __init__(self, authorized_technicians: set[str]) -> None:
        self.authorized_technicians = authorized_technicians

    def connect(self) -> None:
        pass

    def is_technician_authorized(self, technician_id: str) -> bool:
        return technician_id in self.authorized_technicians


def test_with_valid_telegram_id() -> bool:
    provider = MockOTPProvider()
    agent = CabinetAgent(
        agent_id="AGENT-CAB-A",
        cabinet_id="CAB-A",
        store=MockPostgresStore({"7586394272"}),
        otp_provider=provider,
    )
    event = Event(type="TECHNICIAN_ARRIVED", payload={"technician_id": "7586394272"})
    result = agent.handle_event(event, dry_run=False)
    return result["proposal"]["decision"] == "OTP_REQUIRED"


def test_unknown_telegram_id() -> bool:
    provider = MockOTPProvider()
    agent = CabinetAgent(
        agent_id="AGENT-CAB-A",
        cabinet_id="CAB-A",
        store=MockPostgresStore(set()),
        otp_provider=provider,
    )
    event = Event(type="TECHNICIAN_ARRIVED", payload={"technician_id": "9999999999"})
    result = agent.handle_event(event, dry_run=False)
    return result["proposal"]["decision"] == "ACCESS_DENIED"


def test_valid_otp_authentication() -> bool:
    provider = MockOTPProvider()
    provider._codes["7586394272"] = "123456"
    agent = CabinetAgent(
        agent_id="AGENT-CAB-A",
        cabinet_id="CAB-A",
        store=MockPostgresStore({"7586394272"}),
        otp_provider=provider,
    )
    result = provider.verify_otp("7586394272", "123456")
    return result


def test_invalid_otp() -> bool:
    provider = MockOTPProvider()
    provider._codes["7586394272"] = "123456"
    result = provider.verify_otp("7586394272", "000000")
    return not result


if __name__ == "__main__":
    tests = [
        ("VALID TELEGRAM ID", test_with_valid_telegram_id()),
        ("UNKNOWN TELEGRAM ID", test_unknown_telegram_id()),
        ("VALID OTP AUTHENTICATION", test_valid_otp_authentication()),
        ("INVALID OTP", test_invalid_otp()),
    ]
    for name, passed in tests:
        print(f"{name}: {'PASS' if passed else 'FAIL'}")
    raise SystemExit(0 if all(passed for _, passed in tests) else 1)
