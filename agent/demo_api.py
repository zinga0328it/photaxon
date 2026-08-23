from __future__ import annotations

import datetime
import hashlib
import hmac
import json
import os
import secrets
import subprocess
import uuid

from fastapi import FastAPI, HTTPException, status
from pydantic import BaseModel, constr

from engine.postgres_store import PostgresStore
from agent.otp_service import OTPService
from agent.telegram_provider import RealTelegramProvider

TELEGRAM_SECRET_PATH = os.environ.get("TELEGRAM_SECRET_PATH", "secrets/telegram-bot.env")

SIMULATION_STATE = {
    "simulation": True,
    "project": "FiberSpider",
    "status": "JOB_CLOSED",
    "technician": {
        "authenticated": True,
    },
    "ont": {
        "serial": "FS-ONT-DEMO-0001",
        "detected": True,
    },
    "path": [
        "CENTRAL-01",
        "CAB-A",
        "FTTH-LINE-A",
        "SPL-A-01",
        "ROE-LAB-001",
        "FS-ONT-DEMO-0001",
    ],
    "new_resources": ["ROE-LAB-001"],
    "llm": {
        "role": "topology_interpretation",
        "proposal_created": True,
    },
    "backend": {
        "validation": "PASS",
    },
    "provisioning": {
        "mode": "SIMULATION",
        "status": "SUCCESS",
    },
    "customer_line": {
        "simulated_status": "ACTIVE",
    },
    "field_activity": {
        "simulated_status": "CLOSED",
    },
    "final_state": "JOB_CLOSED",
    "notice": "FiberSpider laboratory simulation — no physical FTTH provisioning executed.",
}

app = FastAPI(title="FiberSpider Demo API", version="0.1.0")


class ActivationStartRequest(BaseModel):
    telegram_id: constr(strip_whitespace=True, min_length=5, max_length=32)
    wr: constr(strip_whitespace=True, min_length=3, max_length=128)
    ont_serial: constr(strip_whitespace=True, min_length=3, max_length=64)
    roe: constr(strip_whitespace=True, min_length=3, max_length=64)
    cabinet: constr(strip_whitespace=True, min_length=1, max_length=64)
    splitter: constr(strip_whitespace=True, min_length=1, max_length=64)
    notes: constr(strip_whitespace=True, max_length=512)


class ActivationStartResponse(BaseModel):
    status: str
    activation_id: str
    expires_in: int
    technician: str
    wr_validation: str


OTP_TTL_SECONDS = 300
RATE_LIMIT_SECONDS = 30
LAST_OTP_REQUEST: dict[str, datetime.datetime] = {}


def _hash_otp_with_salt(otp: str, otp_salt: str) -> str:
    return hmac.new(otp_salt.encode("utf-8"), otp.encode("utf-8"), hashlib.sha256).hexdigest()


class ActivationVerifyRequest(BaseModel):
    activation_id: constr(strip_whitespace=True, min_length=1)
    otp: constr(strip_whitespace=True, min_length=6, max_length=6)


class ActivationVerifyResponse(BaseModel):
    status: str
    activation_id: str
    final_state: str | None = None
    simulation_result: dict[str, object] | None = None
    attempts_remaining: int | None = None


def _openstack_command(cmd: list[str]) -> list[dict]:
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "OpenStack command failed")
    return json.loads(result.stdout)


@app.post("/api/activation/start", response_model=ActivationStartResponse)
def activation_start(payload: ActivationStartRequest) -> ActivationStartResponse:
    store = PostgresStore()
    store.connect()

    technician_id = payload.telegram_id
    if not store.is_technician_authorized(technician_id):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail={"status": "ACCESS_DENIED"},
        )

    now = datetime.datetime.now(datetime.timezone.utc)
    last_requested = LAST_OTP_REQUEST.get(technician_id)
    if last_requested and (now - last_requested).total_seconds() < RATE_LIMIT_SECONDS:
        raise HTTPException(
            status_code=status.HTTP_429_TOO_MANY_REQUESTS,
            detail={"status": "TOO_MANY_REQUESTS"},
        )

    activation_id = uuid.uuid4().hex
    challenge_id = uuid.uuid4().hex

    conn = store.connection()
    try:
        with conn.cursor() as cur:
            store.create_activation_request(
                activation_id=activation_id,
                technician_id=technician_id,
                wr=payload.wr,
                ont_serial=payload.ont_serial,
                roe=payload.roe,
                cabinet_id=payload.cabinet,
                splitter_id=payload.splitter,
                notes=payload.notes,
                status="OTP_PENDING",
                wr_validation="NOT_IMPLEMENTED",
            )

            otp_service = OTPService(ttl_seconds=OTP_TTL_SECONDS, max_attempts=5)
            otp_code = otp_service.request_otp(activation_id)
            otp_salt = secrets.token_hex(16)
            otp_hash = hmac.new(otp_salt.encode("utf-8"), otp_code.encode("utf-8"), hashlib.sha256).hexdigest()
            store.create_activation_otp_challenge(
                challenge_id=challenge_id,
                activation_id=activation_id,
                otp_hash=otp_hash,
                otp_salt=otp_salt,
                expires_at=(now + datetime.timedelta(seconds=OTP_TTL_SECONDS)).isoformat(),
            )
        conn.commit()
    except Exception as exc:
        try:
            conn.rollback()
        except Exception:
            pass
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail={"status": "ACTIVATION_DB_ERROR", "reason": str(exc)},
        )

    token = os.environ.get("TELEGRAM_BOT_TOKEN")
    if not token:
        if not os.path.exists(TELEGRAM_SECRET_PATH):
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail={"status": "TELEGRAM_CONFIG_MISSING"},
            )

        with open(TELEGRAM_SECRET_PATH, "r", encoding="utf-8") as secret_file:
            for line in secret_file:
                if line.startswith("TELEGRAM_BOT_TOKEN="):
                    token = line.split("=", 1)[1].strip()
                    os.environ["TELEGRAM_BOT_TOKEN"] = token
                    break

    try:
        provider = RealTelegramProvider()
    except RuntimeError:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail={"status": "TELEGRAM_CONFIG_INVALID"},
        )

    chat_id = provider.find_chat_for_user(technician_id)
    if not chat_id:
        with store.connection() as conn:
            with conn.cursor() as cur:
                cur.execute(
                    "UPDATE activation_requests SET status = %s, updated_at = now() WHERE activation_id = %s;",
                    ("TELEGRAM_FAILED", activation_id),
                )
                cur.execute(
                    "DELETE FROM activation_otp_challenges WHERE challenge_id = %s;",
                    (challenge_id,),
                )
            conn.commit()
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={"status": "TELEGRAM_CHAT_NOT_REGISTERED"},
        )

    message = (
        "FiberSpider / Photaxon\n\n"
        f"Codice OTP per attivazione: {otp_code}\n\n"
        f"Validità: {OTP_TTL_SECONDS // 60} minuti\n\n"
        f"WR: {payload.wr}"
    )
    try:
        provider.send_message(chat_id, message)
    except Exception as exc:
        with store.connection() as conn:
            with conn.cursor() as cur:
                cur.execute(
                    "UPDATE activation_requests SET status = %s, updated_at = now() WHERE activation_id = %s;",
                    ("TELEGRAM_FAILED", activation_id),
                )
                cur.execute(
                    "DELETE FROM activation_otp_challenges WHERE challenge_id = %s;",
                    (challenge_id,),
                )
            conn.commit()
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail={"status": "TELEGRAM_SEND_FAILED", "reason": str(exc)},
        )
    LAST_OTP_REQUEST[technician_id] = now

    return ActivationStartResponse(
        status="OTP_SENT",
        activation_id=activation_id,
        expires_in=OTP_TTL_SECONDS,
        technician="AUTHORIZED",
        wr_validation="NOT_IMPLEMENTED",
    )


@app.post("/api/activation/verify", response_model=ActivationVerifyResponse)
def activation_verify(payload: ActivationVerifyRequest) -> ActivationVerifyResponse:
    store = PostgresStore()
    store.connect()
    conn = store.connection()
    now = datetime.datetime.now(datetime.timezone.utc)

    with conn.cursor() as cur:
        cur.execute(
            "SELECT activation_id, status FROM activation_requests WHERE activation_id = %s;",
            (payload.activation_id,),
        )
        activation = cur.fetchone()

    if not activation:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail={"status": "ACTIVATION_NOT_FOUND"},
        )

    if activation["status"] != "OTP_PENDING":
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={"status": "OTP_INVALID"},
        )

    with conn.cursor() as cur:
        cur.execute(
            "SELECT challenge_id, otp_hash, otp_salt, expires_at, attempts, consumed_at FROM activation_otp_challenges WHERE activation_id = %s;",
            (payload.activation_id,),
        )
        challenge = cur.fetchone()

    if not challenge or challenge["consumed_at"] is not None:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={"status": "OTP_INVALID"},
        )

    if challenge["expires_at"] <= now:
        with conn.cursor() as cur:
            cur.execute(
                "UPDATE activation_requests SET status = %s, updated_at = now() WHERE activation_id = %s;",
                ("OTP_EXPIRED", payload.activation_id),
            )
            conn.commit()
        return ActivationVerifyResponse(
            status="OTP_EXPIRED",
            activation_id=payload.activation_id,
        )

    if challenge["attempts"] >= 5:
        with conn.cursor() as cur:
            cur.execute(
                "UPDATE activation_requests SET status = %s, updated_at = now() WHERE activation_id = %s;",
                ("OTP_LOCKED", payload.activation_id),
            )
            conn.commit()
        return ActivationVerifyResponse(
            status="OTP_LOCKED",
            activation_id=payload.activation_id,
        )

    otp_hash = _hash_otp_with_salt(payload.otp, challenge["otp_salt"])
    if not hmac.compare_digest(otp_hash, challenge["otp_hash"]):
        attempts_remaining = max(0, 5 - (challenge["attempts"] + 1))
        with conn.cursor() as cur:
            cur.execute(
                "UPDATE activation_otp_challenges SET attempts = attempts + 1 WHERE challenge_id = %s;",
                (challenge["challenge_id"],),
            )
            conn.commit()
        return ActivationVerifyResponse(
            status="OTP_INVALID",
            activation_id=payload.activation_id,
            attempts_remaining=attempts_remaining,
        )

    try:
        with conn.cursor() as cur:
            store.consume_activation_otp_challenge(challenge["challenge_id"])
            store.mark_activation_otp_verified(challenge["challenge_id"])
            conn.commit()
    except Exception as exc:
        conn.rollback()
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail={"status": "ACTIVATION_DB_ERROR", "reason": str(exc)},
        )

    simulation_result = {
        "technician_authorized": True,
        "otp_verified": True,
        "ont_detected": True,
        "field_path_confirmed": True,
        "provisioning_plan": {
            "created": True,
            "status": "SIMULATED",
        },
        "provisioning": {
            "executed": False,
            "result": "SIMULATED",
        },
        "customer_line": {
            "status": "ACTIVE",
            "simulated": True,
        },
        "final_state": "JOB_CLOSED",
    }

    try:
        with conn.cursor() as cur:
            store.complete_activation_simulation(payload.activation_id, simulation_result)
            conn.commit()
    except Exception as exc:
        conn.rollback()
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail={"status": "SIMULATION_FAILED", "reason": str(exc)},
        )

    return ActivationVerifyResponse(
        status="SIMULATION_COMPLETED",
        activation_id=payload.activation_id,
        final_state="JOB_CLOSED",
        simulation_result=simulation_result,
    )


@app.get("/api/health")
def health() -> dict[str, str]:
    return {"status": "ok", "service": "fiberspider-demo-api"}


@app.get("/api/lab/demo")
def demo() -> dict:
    return SIMULATION_STATE


def _openstack_command(cmd: list[str]) -> list[dict]:
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "OpenStack command failed")
    return json.loads(result.stdout)


@app.get("/api/lab/openstack")
def openstack_status() -> dict:
    try:
        servers = _openstack_command([
            "openstack",
            "--os-cloud",
            "devstack-admin",
            "server",
            "list",
            "-f",
            "json",
        ])
        networks = _openstack_command([
            "openstack",
            "--os-cloud",
            "devstack-admin",
            "network",
            "list",
            "-f",
            "json",
        ])
    except Exception:
        return {
            "openstack": {"status": "OFFLINE"},
            "servers": {
                "CAB-A": {"status": "UNKNOWN"},
                "CAB-B": {"status": "UNKNOWN"},
                "CAB-C": {"status": "UNKNOWN"},
                "FS-TEST-01": {"status": "UNKNOWN"},
            },
            "networks": {
                "FS-MGMT": {"present": False},
                "FS-LINE-A": {"present": False},
                "FS-LINE-B": {"present": False},
                "FS-LINE-C": {"present": False},
            },
        }

    server_map = {s["Name"]: s for s in servers}
    network_names = {n["Name"] for n in networks}

    def server_info(name: str) -> dict[str, str]:
        server = server_map.get(name)
        if not server:
            return {"status": "OTHER"}
        status = server.get("Status", "OTHER")
        return {"status": status}

    return {
        "openstack": {"status": "ONLINE"},
        "servers": {
            "CAB-A": server_info("CAB-A"),
            "CAB-B": server_info("CAB-B"),
            "CAB-C": server_info("CAB-C"),
            "FS-TEST-01": server_info("FS-TEST-01"),
        },
        "networks": {
            "FS-MGMT": {"present": "FS-MGMT" in network_names},
            "FS-LINE-A": {"present": "FS-LINE-A" in network_names},
            "FS-LINE-B": {"present": "FS-LINE-B" in network_names},
            "FS-LINE-C": {"present": "FS-LINE-C" in network_names},
        },
    }
