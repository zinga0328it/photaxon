from __future__ import annotations

import datetime
import time

from agent.otp_service import OTPService


def test_generate_otp() -> bool:
    service = OTPService(ttl_seconds=300)
    otp = service.request_otp("7586394272")
    return len(otp) == 6 and otp.isdigit()


def test_wrong_otp() -> bool:
    service = OTPService(ttl_seconds=300)
    service.request_otp("7586394272")
    return service.verify_otp("7586394272", "000000") == "OTP_INVALID"


def test_correct_otp_one_time_use() -> bool:
    service = OTPService(ttl_seconds=300)
    otp = service.request_otp("7586394272")
    result = service.verify_otp("7586394272", otp)
    reuse_result = service.verify_otp("7586394272", otp)
    return result == "AUTHENTICATED" and reuse_result == "OTP_INVALID"


def test_expired_otp() -> bool:
    service = OTPService(ttl_seconds=1)
    otp = service.request_otp("7586394272")
    time.sleep(2)
    return service.verify_otp("7586394272", otp) == "OTP_EXPIRED"


def test_lockout_after_three_incorrect_attempts() -> bool:
    service = OTPService(ttl_seconds=300)
    service.request_otp("7586394272")
    service.verify_otp("7586394272", "000000")
    service.verify_otp("7586394272", "000000")
    result = service.verify_otp("7586394272", "000000")
    return result == "OTP_LOCKED"


if __name__ == "__main__":
    tests = [
        ("GENERATE OTP", test_generate_otp()),
        ("WRONG OTP", test_wrong_otp()),
        ("CORRECT OTP ONE TIME USE", test_correct_otp_one_time_use()),
        ("EXPIRED OTP", test_expired_otp()),
        ("LOCKOUT AFTER THREE INCORRECT ATTEMPTS", test_lockout_after_three_incorrect_attempts()),
    ]
    for name, passed in tests:
        print(f"{name}: {'PASS' if passed else 'FAIL'}")
    raise SystemExit(0 if all(passed for _, passed in tests) else 1)
