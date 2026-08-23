from __future__ import annotations

from abc import ABC, abstractmethod


class OTPProvider(ABC):
    """Abstract OTP provider interface for future Telegram integration."""

    @abstractmethod
    def request_otp(self, technician_id: str) -> str:
        raise NotImplementedError

    @abstractmethod
    def verify_otp(self, technician_id: str, otp: str) -> bool:
        raise NotImplementedError


class MockOTPProvider(OTPProvider):
    """Mock OTP provider used for local simulation and unit tests."""

    def __init__(self) -> None:
        self._codes: dict[str, str] = {}

    def request_otp(self, technician_id: str) -> str:
        code = self._codes.get(technician_id, "000000")
        self._codes[technician_id] = code
        return code

    def verify_otp(self, technician_id: str, otp: str) -> bool:
        return self._codes.get(technician_id) == otp
