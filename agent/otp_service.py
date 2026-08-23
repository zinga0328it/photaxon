from __future__ import annotations

import datetime
import hashlib
import hmac
import secrets
from dataclasses import dataclass
from typing import Any


@dataclass
class OTPEntry:
    otp_hash: str
    expires_at: datetime.datetime
    attempts: int
    locked: bool = False


class OTPService:
    """In-memory OTP service with secure digest storage."""

    def __init__(self, ttl_seconds: int = 300, max_attempts: int = 3) -> None:
        self.ttl_seconds = ttl_seconds
        self.max_attempts = max_attempts
        self._store: dict[str, OTPEntry] = {}

    def _hash_otp(self, otp: str) -> str:
        digest = hashlib.sha256(otp.encode("utf-8")).hexdigest()
        return digest

    def request_otp(self, technician_id: str) -> str:
        otp = f"{secrets.randbelow(10**6):06d}"
        otp_hash = self._hash_otp(otp)
        expires_at = datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(seconds=self.ttl_seconds)
        self._store[technician_id] = OTPEntry(otp_hash=otp_hash, expires_at=expires_at, attempts=0, locked=False)
        return otp

    def verify_otp(self, technician_id: str, otp: str) -> str:
        entry = self._store.get(technician_id)
        if entry is None:
            return "OTP_INVALID"

        now = datetime.datetime.now(datetime.timezone.utc)
        if entry.locked:
            return "OTP_LOCKED"
        if now > entry.expires_at:
            self._store.pop(technician_id, None)
            return "OTP_EXPIRED"

        otp_hash = self._hash_otp(otp)
        if hmac.compare_digest(otp_hash, entry.otp_hash):
            self._store.pop(technician_id, None)
            return "AUTHENTICATED"

        entry.attempts += 1
        if entry.attempts >= self.max_attempts:
            entry.locked = True
            return "OTP_LOCKED"

        return "OTP_INVALID"

    def get_status(self, technician_id: str) -> str | None:
        entry = self._store.get(technician_id)
        if not entry:
            return None
        now = datetime.datetime.now(datetime.timezone.utc)
        if entry.locked:
            return "OTP_LOCKED"
        if now > entry.expires_at:
            return "OTP_EXPIRED"
        return "OTP_PENDING"
