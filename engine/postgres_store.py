from __future__ import annotations

import os
from typing import Any

import json
import psycopg
from psycopg.rows import dict_row
from psycopg.types.json import Json


class PostgresStore:
    """Read-oriented PostgreSQL adapter for the FiberSpider lab topology on AAA."""

    def __init__(self) -> None:
        self._host = os.environ["FIBERSPIDER_DB_HOST"]
        self._port = os.environ["FIBERSPIDER_DB_PORT"]
        self._name = os.environ["FIBERSPIDER_DB_NAME"]
        self._user = os.environ["FIBERSPIDER_DB_USER"]
        self._password = os.environ["FIBERSPIDER_DB_PASSWORD"]
        self._conn: psycopg.Connection | None = None

    def connect(self) -> None:
        if self._conn is not None and not self._conn.closed:
            return
        self._conn = psycopg.connect(
            host=self._host,
            port=self._port,
            dbname=self._name,
            user=self._user,
            password=self._password,
            row_factory=dict_row,
        )

    def close(self) -> None:
        if self._conn is not None and not self._conn.closed:
            self._conn.close()
        self._conn = None

    def _connection(self) -> psycopg.Connection:
        if self._conn is None or self._conn.closed:
            raise RuntimeError("PostgresStore is not connected: call connect() first")
        return self._conn

    def connection(self) -> psycopg.Connection:
        """Expose the active connection for callers that need direct transaction control."""
        return self._connection()

    def _open_lease_connection(self) -> psycopg.Connection:
        return psycopg.connect(
            host=self._host,
            port=self._port,
            dbname=self._name,
            user=self._user,
            password=self._password,
            row_factory=dict_row,
        )

    def healthcheck(self) -> dict[str, Any]:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "SELECT current_database() AS database, current_user AS \"user\", "
                "inet_client_addr() AS client_addr, inet_server_addr() AS server_addr;"
            )
            row = cur.fetchone()
        return dict(row) if row else {}

    def get_cabinets(self) -> list[dict[str, Any]]:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute("SELECT cabinet_id, status, created_at, updated_at FROM cabinets ORDER BY cabinet_id;")
            return [dict(row) for row in cur.fetchall()]

    def get_ftth_lines(self) -> list[dict[str, Any]]:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "SELECT line_id, cabinet_id, openstack_network_name, status, created_at, updated_at "
                "FROM ftth_lines ORDER BY line_id;"
            )
            return [dict(row) for row in cur.fetchall()]

    def get_splitters(self) -> list[dict[str, Any]]:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "SELECT splitter_id, cabinet_id, line_id, ratio, capacity_total, "
                "secondary_splitter_allowed, status, created_at, updated_at "
                "FROM splitters ORDER BY splitter_id;"
            )
            return [dict(row) for row in cur.fetchall()]

    def get_agents(self) -> list[dict[str, Any]]:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "SELECT agent_id, cabinet_id, role, status, runtime_provider, runtime_id, mgmt_ip, last_seen_at, created_at, updated_at "
                "FROM agents ORDER BY agent_id;"
            )
            return [dict(row) for row in cur.fetchall()]

    def get_agent(self, agent_id: str) -> dict[str, Any] | None:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "SELECT agent_id, cabinet_id, role, status, runtime_provider, runtime_id, mgmt_ip, last_seen_at, created_at, updated_at "
                "FROM agents WHERE agent_id = %s;",
                (agent_id,),
            )
            row = cur.fetchone()
        return dict(row) if row else None

    def is_technician_authorized(self, technician_id: str) -> bool:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "SELECT status FROM technicians WHERE technician_id = %s;",
                (technician_id,),
            )
            row = cur.fetchone()
        if not row:
            return False
        return row["status"].upper() == "ACTIVE"

    def update_agent_runtime(
        self,
        agent_id: str,
        runtime_provider: str | None,
        runtime_id: str | None,
        mgmt_ip: str | None,
    ) -> None:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "UPDATE agents SET runtime_provider = %s, runtime_id = %s, mgmt_ip = %s, updated_at = now() WHERE agent_id = %s;",
                (runtime_provider, runtime_id, mgmt_ip, agent_id),
            )

    def update_agent_last_seen(self, agent_id: str) -> None:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "UPDATE agents SET last_seen_at = now(), updated_at = now() WHERE agent_id = %s;",
                (agent_id,),
            )

    def get_resource_state(self, resource_id: str) -> str | None:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "SELECT resource_state FROM resource_states WHERE resource_id = %s;",
                (resource_id,),
            )
            row = cur.fetchone()
        return row["resource_state"] if row else None

    def get_resource_metadata(self, resource_id: str) -> dict[str, Any] | None:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "SELECT metadata FROM resource_states WHERE resource_id = %s;",
                (resource_id,),
            )
            row = cur.fetchone()
        return dict(row["metadata"]) if row else None

    def create_resource_state(
        self,
        resource_id: str,
        resource_kind: str,
        resource_state: str,
        owner_transaction_id: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> None:
        metadata = metadata or {}
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "INSERT INTO resource_states (resource_id, resource_kind, resource_state, owner_transaction_id, revision, metadata) "
                "VALUES (%s, %s, %s, %s, 1, %s) "
                "ON CONFLICT (resource_id) DO NOTHING;",
                (resource_id, resource_kind, resource_state, owner_transaction_id, Json(metadata)),
            )

    def update_resource_state(
        self,
        resource_id: str,
        resource_state: str,
        owner_transaction_id: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> None:
        metadata = metadata if metadata is not None else {}
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "UPDATE resource_states SET resource_state = %s, owner_transaction_id = %s, metadata = %s, revision = revision + 1, updated_at = now() WHERE resource_id = %s;",
                (resource_state, owner_transaction_id, Json(metadata), resource_id),
            )

    def update_resource_revision(self, resource_id: str, revision: int) -> None:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "UPDATE resource_states SET revision = %s, updated_at = now() WHERE resource_id = %s;",
                (revision, resource_id),
            )

    def acquire_resource_lease(
        self,
        lease_id: str,
        resource_kind: str,
        resource_id: str,
        lease_owner: str,
        lease_token: str,
        expires_at: str,
        transaction_id: str | None = None,
    ) -> None:
        conn = self._open_lease_connection()
        try:
            with conn.cursor() as cur:
                cur.execute(
                    "INSERT INTO resource_leases (lease_id, resource_kind, resource_id, transaction_id, lease_owner, lease_token, expires_at) "
                    "VALUES (%s, %s, %s, %s, %s, %s, %s);",
                    (lease_id, resource_kind, resource_id, transaction_id, lease_owner, lease_token, expires_at),
                )
            conn.commit()
        finally:
            conn.close()

    def release_resource_lease(self, lease_id: str, lease_token: str) -> bool:
        conn = self._open_lease_connection()
        try:
            with conn.cursor() as cur:
                cur.execute(
                    "UPDATE resource_leases SET released_at = now() WHERE lease_id = %s AND lease_token = %s AND released_at IS NULL;",
                    (lease_id, lease_token),
                )
                result = cur.rowcount == 1
            conn.commit()
            return result
        finally:
            conn.close()

    def bind_resource_lease_to_transaction(self, lease_id: str, lease_token: str, transaction_id: str) -> bool:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "UPDATE resource_leases SET transaction_id = %s "
                "WHERE lease_id = %s AND lease_token = %s AND released_at IS NULL AND transaction_id IS NULL;",
                (transaction_id, lease_id, lease_token),
            )
            return cur.rowcount == 1

    def __enter__(self) -> "PostgresStore":
        self.connect()
        return self

    def __exit__(self, exc_type, exc_value, traceback) -> None:
        self.close()

    def create_activation_request(
        self,
        activation_id: str,
        technician_id: str,
        wr: str,
        ont_serial: str,
        roe: str | None,
        cabinet_id: str | None,
        splitter_id: str | None,
        notes: str | None,
        status: str,
        wr_validation: str = "NOT_IMPLEMENTED",
    ) -> None:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "INSERT INTO activation_requests (activation_id, technician_id, wr, ont_serial, roe, cabinet_id, splitter_id, notes, status, wr_validation) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s);",
                (activation_id, technician_id, wr, ont_serial, roe, cabinet_id, splitter_id, notes, status, wr_validation),
            )

    def get_activation_request(self, activation_id: str) -> dict[str, Any] | None:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "SELECT activation_id, technician_id, wr, ont_serial, roe, cabinet_id, splitter_id, notes, status, wr_validation, simulation_result, otp_verified_at, completed_at, created_at, updated_at FROM activation_requests WHERE activation_id = %s;",
                (activation_id,),
            )
            row = cur.fetchone()
        return dict(row) if row else None

    def create_activation_otp_challenge(
        self,
        challenge_id: str,
        activation_id: str,
        otp_hash: str,
        otp_salt: str,
        expires_at: str,
    ) -> None:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "INSERT INTO activation_otp_challenges (challenge_id, activation_id, otp_hash, otp_salt, expires_at) VALUES (%s, %s, %s, %s, %s);",
                (challenge_id, activation_id, otp_hash, otp_salt, expires_at),
            )

    def get_activation_otp_challenge(self, challenge_id: str) -> dict[str, Any] | None:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "SELECT challenge_id, activation_id, otp_hash, otp_salt, expires_at, attempts, consumed_at, created_at FROM activation_otp_challenges WHERE challenge_id = %s;",
                (challenge_id,),
            )
            row = cur.fetchone()
        return dict(row) if row else None

    def increment_activation_otp_attempts(self, challenge_id: str) -> None:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "UPDATE activation_otp_challenges SET attempts = attempts + 1 WHERE challenge_id = %s;",
                (challenge_id,),
            )

    def consume_activation_otp_challenge(self, challenge_id: str) -> None:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "UPDATE activation_otp_challenges SET consumed_at = now() WHERE challenge_id = %s;",
                (challenge_id,),
            )

    def mark_activation_otp_verified(self, challenge_id: str) -> None:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "UPDATE activation_otp_challenges SET consumed_at = now() WHERE challenge_id = %s;",
                (challenge_id,),
            )
            cur.execute(
                "UPDATE activation_requests SET otp_verified_at = now(), updated_at = now() WHERE activation_id = (SELECT activation_id FROM activation_otp_challenges WHERE challenge_id = %s);",
                (challenge_id,),
            )

    def complete_activation_simulation(
        self,
        activation_id: str,
        simulation_result: dict[str, Any],
    ) -> None:
        conn = self._connection()
        with conn.cursor() as cur:
            cur.execute(
                "UPDATE activation_requests SET simulation_result = %s, status = %s, completed_at = now(), updated_at = now() WHERE activation_id = %s;",
                (Json(simulation_result), "SIMULATION_COMPLETED", activation_id),
            )
