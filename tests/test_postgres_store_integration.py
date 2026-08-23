import os
import unittest
import uuid

import psycopg

from engine.postgres_store import PostgresStore


@unittest.skipUnless(os.environ.get("FIBERSPIDER_RUN_DB_TESTS") == "1", "requires FIBERSPIDER_RUN_DB_TESTS=1")
class PostgresStoreIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.store = PostgresStore()
        self.store.connect()
        self._created_lease_ids = set()

    def tearDown(self):
        conn = self.store._connection()
        if conn.info.transaction_status in (2, 3):
            conn.rollback()
        self._cleanup_test_leases()
        self.store.close()

    def _new_connection(self):
        return psycopg.connect(
            host=os.environ["FIBERSPIDER_DB_HOST"],
            port=os.environ["FIBERSPIDER_DB_PORT"],
            dbname=os.environ["FIBERSPIDER_DB_NAME"],
            user=os.environ["FIBERSPIDER_DB_USER"],
            password=os.environ["FIBERSPIDER_DB_PASSWORD"],
            row_factory=psycopg.rows.dict_row,
        )

    def _cleanup_test_leases(self, resource_id: str | None = None):
        if not self._created_lease_ids:
            return
        with self._new_connection() as conn:
            with conn.cursor() as cur:
                cur.execute(
                    "DELETE FROM resource_leases WHERE lease_id = ANY(%s);",
                    (list(self._created_lease_ids),),
                )
            conn.commit()

    def _record_lease_id(self, lease_id: str) -> str:
        self._created_lease_ids.add(lease_id)
        return lease_id

    def _existing_port_id(self) -> str:
        with self.store._connection().cursor() as cur:
            cur.execute("SELECT port_id FROM ports LIMIT 1;")
            row = cur.fetchone()
        if not row:
            raise RuntimeError("No port_id found in ports table")
        return row["port_id"]

    def test_healthcheck_reports_database_and_user(self):
        health = self.store.healthcheck()
        self.assertEqual(health["database"], "fiberspider")
        self.assertEqual(health["user"], "fiberspider_app")
        self.assertIn("client_addr", health)
        self.assertIn("server_addr", health)

    def test_cabinets_seeded_lab_topology(self):
        cabinets = {row["cabinet_id"] for row in self.store.get_cabinets()}
        self.assertEqual({"CAB-A", "CAB-B", "CAB-C"}, cabinets & {"CAB-A", "CAB-B", "CAB-C"})
        self.assertEqual(len(self.store.get_cabinets()), 3)

    def test_ftth_lines_openstack_mapping(self):
        lines = {row["line_id"]: row["openstack_network_name"] for row in self.store.get_ftth_lines()}
        self.assertEqual(lines["FTTH-LINE-A"], "FS-LINE-A")
        self.assertEqual(lines["FTTH-LINE-B"], "FS-LINE-B")
        self.assertEqual(lines["FTTH-LINE-C"], "FS-LINE-C")
        self.assertEqual(len(self.store.get_ftth_lines()), 3)

    def test_splitters_seeded_lab_topology(self):
        splitters = {row["splitter_id"] for row in self.store.get_splitters()}
        self.assertEqual({"SPL-A-01", "SPL-B-01", "SPL-C-01"}, splitters & {"SPL-A-01", "SPL-B-01", "SPL-C-01"})
        self.assertEqual(len(self.store.get_splitters()), 3)

    def test_agents_seeded_lab_topology(self):
        agents = {row["agent_id"] for row in self.store.get_agents()}
        self.assertEqual({"AGENT-CAB-A", "AGENT-CAB-B", "AGENT-CAB-C"}, agents & {"AGENT-CAB-A", "AGENT-CAB-B", "AGENT-CAB-C"})
        self.assertEqual(len(self.store.get_agents()), 3)

    def test_get_agent_returns_agent_row(self):
        agent = self.store.get_agent("AGENT-CAB-A")
        self.assertIsNotNone(agent)
        self.assertEqual(agent["agent_id"], "AGENT-CAB-A")
        self.assertEqual(agent["cabinet_id"], "CAB-A")
        self.assertIn("status", agent)

    def test_update_agent_runtime_and_last_seen(self):
        agent_id = "AGENT-CAB-A"
        self.store.update_agent_runtime(agent_id, "openstack", "TEST-UUID", "10.50.0.159")
        self.store.update_agent_last_seen(agent_id)
        agent = self.store.get_agent(agent_id)
        self.assertIsNotNone(agent)
        self.assertEqual(agent["runtime_provider"], "openstack")
        self.assertEqual(agent["runtime_id"], "TEST-UUID")
        self.assertEqual(str(agent["mgmt_ip"]), "10.50.0.159")
        self.assertIsNotNone(agent["last_seen_at"])

    def test_transaction_rollback_leaves_no_permanent_data(self):
        technician_id = f"TEST-TECH-ROLLBACK-{uuid.uuid4().hex[:8]}"
        conn = self.store._connection()
        with conn.transaction() as tx:
            conn.execute(
                "INSERT INTO technicians (technician_id, display_name, status) VALUES (%s, %s, %s);",
                (technician_id, "FiberSpider Rollback Test", "ACTIVE"),
            )
            with conn.cursor() as cur:
                cur.execute("SELECT technician_id FROM technicians WHERE technician_id = %s;", (technician_id,))
                self.assertIsNotNone(cur.fetchone())
            raise psycopg.Rollback(tx)

        with conn.cursor() as cur:
            cur.execute("SELECT technician_id FROM technicians WHERE technician_id = %s;", (technician_id,))
            self.assertIsNone(cur.fetchone())

    def test_acquire_resource_lease_blocks_duplicate_active_lease(self):
        lease_id_a = self._record_lease_id(f"TEST-LEASE-A-{uuid.uuid4().hex[:8]}")
        lease_id_b = self._record_lease_id(f"TEST-LEASE-B-{uuid.uuid4().hex[:8]}")
        resource_kind = "PORT"
        resource_id = f"TEST-RESOURCE-LEASER-{uuid.uuid4().hex[:8]}"
        lease_owner_a = "OWNER-A"
        lease_owner_b = "OWNER-B"
        lease_token_a = f"TOKEN-A-{uuid.uuid4().hex[:8]}"
        lease_token_b = f"TOKEN-B-{uuid.uuid4().hex[:8]}"
        expires_at = "2099-12-31T23:59:59Z"

        self.store.acquire_resource_lease(
            lease_id_a,
            resource_kind,
            resource_id,
            lease_owner_a,
            lease_token_a,
            expires_at,
            transaction_id=None,
        )
        with self.assertRaises(psycopg.errors.UniqueViolation):
            self.store.acquire_resource_lease(
                lease_id_b,
                resource_kind,
                resource_id,
                lease_owner_b,
                lease_token_b,
                expires_at,
                transaction_id=None,
            )

        with self._new_connection() as other:
            with other.cursor() as cur:
                cur.execute("SELECT count(*) FROM resource_leases WHERE resource_id = %s AND released_at IS NULL;", (resource_id,))
                row = cur.fetchone()
        self.assertEqual(row["count"], 1)

    def test_acquire_resource_lease_uses_dedicated_connection_and_is_visible(self):
        lease_id = self._record_lease_id(f"TEST-LEASE-VIS-{uuid.uuid4().hex[:8]}")
        resource_kind = "PORT"
        resource_id = f"TEST-RESOURCE-LEASE-VIS-{uuid.uuid4().hex[:8]}"
        lease_owner = "OWNER-VIS"
        lease_token = f"TOKEN-VIS-{uuid.uuid4().hex[:8]}"
        expires_at = "2099-12-31T23:59:59Z"

        conn = self.store._connection()
        self.assertEqual(conn.info.transaction_status, 0)
        self.store.acquire_resource_lease(
            lease_id,
            resource_kind,
            resource_id,
            lease_owner,
            lease_token,
            expires_at,
            transaction_id=None,
        )
        self.assertEqual(conn.info.transaction_status, 0)

        with self._new_connection() as other:
            with other.cursor() as cur:
                cur.execute("SELECT lease_id FROM resource_leases WHERE lease_id = %s;", (lease_id,))
                row = cur.fetchone()
            self.assertIsNotNone(row)
            self.assertEqual(row["lease_id"], lease_id)

    def test_acquire_resource_lease_is_isolated_from_shared_uncommitted_transaction(self):
        lease_id = self._record_lease_id(f"TEST-LEASE-ISO-{uuid.uuid4().hex[:8]}")
        resource_kind = "PORT"
        resource_id = f"TEST-RESOURCE-LEASE-ISO-{uuid.uuid4().hex[:8] }"
        lease_owner = "OWNER-ISO"
        lease_token = f"TOKEN-ISO-{uuid.uuid4().hex[:8]}"
        expires_at = "2099-12-31T23:59:59Z"
        uncommitted_technician = f"TEST-TECH-UNCOM-{uuid.uuid4().hex[:8]}"

        conn = self.store._connection()
        with conn.transaction() as tx:
            conn.execute(
                "INSERT INTO technicians (technician_id, display_name, status) VALUES (%s, %s, %s);",
                (uncommitted_technician, "Uncommitted Test", "ACTIVE"),
            )
            self.store.acquire_resource_lease(
                lease_id,
                resource_kind,
                resource_id,
                lease_owner,
                lease_token,
                expires_at,
                transaction_id=None,
            )
            self.assertEqual(conn.info.transaction_status, 2)
            with self._new_connection() as other:
                with other.cursor() as cur:
                    cur.execute("SELECT lease_id FROM resource_leases WHERE lease_id = %s;", (lease_id,))
                    lease_row = cur.fetchone()
                    cur.execute("SELECT technician_id FROM technicians WHERE technician_id = %s;", (uncommitted_technician,))
                    tech_row = cur.fetchone()
            self.assertIsNotNone(lease_row)
            self.assertIsNone(tech_row)
            raise psycopg.Rollback(tx)

    def test_release_resource_lease_is_visible_and_allows_new_lease(self):
        lease_id = self._record_lease_id(f"TEST-LEASE-REL-VIS-{uuid.uuid4().hex[:8]}")
        resource_kind = "PORT"
        resource_id = f"TEST-RESOURCE-LEASE-REL-VIS-{uuid.uuid4().hex[:8]}"
        lease_owner = "OWNER-REL-VIS"
        lease_token = f"TOKEN-REL-VIS-{uuid.uuid4().hex[:8]}"
        expires_at = "2099-12-31T23:59:59Z"

        self.store.acquire_resource_lease(
            lease_id,
            resource_kind,
            resource_id,
            lease_owner,
            lease_token,
            expires_at,
            transaction_id=None,
        )
        self.assertTrue(self.store.release_resource_lease(lease_id, lease_token))

        with self._new_connection() as other:
            with other.cursor() as cur:
                cur.execute("SELECT released_at FROM resource_leases WHERE lease_id = %s;", (lease_id,))
                row = cur.fetchone()
            self.assertIsNotNone(row)
            self.assertIsNotNone(row["released_at"])

        next_lease_id = self._record_lease_id(f"TEST-LEASE-REL-VIS2-{uuid.uuid4().hex[:8]}")
        next_lease_token = f"TOKEN-REL-VIS2-{uuid.uuid4().hex[:8]}"
        self.store.acquire_resource_lease(
            next_lease_id,
            resource_kind,
            resource_id,
            lease_owner,
            next_lease_token,
            expires_at,
            transaction_id=None,
        )
        with self._new_connection() as other:
            with other.cursor() as cur:
                cur.execute(
                    "SELECT count(*) FROM resource_leases WHERE resource_id = %s AND released_at IS NULL;",
                    (resource_id,),
                )
                row = cur.fetchone()
        self.assertEqual(row["count"], 1)

    def test_release_resource_lease_marks_released_at(self):
        lease_id = self._record_lease_id(f"TEST-LEASE-REL-{uuid.uuid4().hex[:8]}")
        resource_kind = "PORT"
        resource_id = f"TEST-RESOURCE-LEASE-REL-{uuid.uuid4().hex[:8]}"
        lease_owner = "OWNER-REL"
        lease_token = f"TOKEN-REL-{uuid.uuid4().hex[:8]}"
        expires_at = "2099-12-31T23:59:59Z"

        conn = self.store._connection()
        with conn.transaction() as tx:
            self.store.acquire_resource_lease(
                lease_id,
                resource_kind,
                resource_id,
                lease_owner,
                lease_token,
                expires_at,
                transaction_id=None,
            )
            result = self.store.release_resource_lease(lease_id, lease_token)
            self.assertTrue(result)
            with conn.cursor() as cur:
                cur.execute("SELECT lease_id, resource_id, released_at FROM resource_leases WHERE lease_id = %s;", (lease_id,))
                row = cur.fetchone()
            self.assertEqual(row["lease_id"], lease_id)
            self.assertEqual(row["resource_id"], resource_id)
            self.assertIsNotNone(row["released_at"])
            raise psycopg.Rollback(tx)

    def test_bind_resource_lease_to_transaction_with_correct_token(self):
        lease_id = self._record_lease_id(f"TEST-LEASE-BIND-{uuid.uuid4().hex[:8]}")
        resource_kind = "PORT"
        resource_id = self._existing_port_id()
        lease_owner = "OWNER-BIND"
        lease_token = f"TOKEN-BIND-{uuid.uuid4().hex[:8]}"
        expires_at = "2099-12-31T23:59:59Z"
        transaction_id = f"TEST-TX-BIND-{uuid.uuid4().hex[:8]}"

        self.store.acquire_resource_lease(
            lease_id,
            resource_kind,
            resource_id,
            lease_owner,
            lease_token,
            expires_at,
            transaction_id=None,
        )

        conn = self.store._connection()
        with conn.transaction():
            conn.execute(
                "INSERT INTO transactions (transaction_id, idempotency_key, correlation_id, port_id, status, validated_by) VALUES (%s, %s, %s, %s, %s, %s);",
                (transaction_id, transaction_id, transaction_id, resource_id, "RESERVED", "test"),
            )
            result = self.store.bind_resource_lease_to_transaction(lease_id, lease_token, transaction_id)
            self.assertTrue(result)

        with self._new_connection() as other:
            with other.cursor() as cur:
                cur.execute("SELECT transaction_id FROM resource_leases WHERE lease_id = %s;", (lease_id,))
                row = cur.fetchone()
            self.assertIsNotNone(row)
            self.assertEqual(row["transaction_id"], transaction_id)

    def test_bind_resource_lease_to_transaction_with_wrong_token(self):
        lease_id = self._record_lease_id(f"TEST-LEASE-BIND-WRONG-{uuid.uuid4().hex[:8]}")
        resource_kind = "PORT"
        resource_id = self._existing_port_id()
        lease_owner = "OWNER-BIND-WRONG"
        lease_token = f"TOKEN-BIND-WRONG-{uuid.uuid4().hex[:8]}"
        expires_at = "2099-12-31T23:59:59Z"
        transaction_id = f"TEST-TX-BIND-WRONG-{uuid.uuid4().hex[:8]}"

        self.store.acquire_resource_lease(
            lease_id,
            resource_kind,
            resource_id,
            lease_owner,
            lease_token,
            expires_at,
            transaction_id=None,
        )

        conn = self.store._connection()
        with conn.transaction():
            conn.execute(
                "INSERT INTO transactions (transaction_id, idempotency_key, correlation_id, port_id, status, validated_by) VALUES (%s, %s, %s, %s, %s, %s);",
                (transaction_id, transaction_id, transaction_id, resource_id, "RESERVED", "test"),
            )
            result = self.store.bind_resource_lease_to_transaction(lease_id, "WRONG-TOKEN", transaction_id)
            self.assertFalse(result)

        with self._new_connection() as other:
            with other.cursor() as cur:
                cur.execute("SELECT transaction_id FROM resource_leases WHERE lease_id = %s;", (lease_id,))
                row = cur.fetchone()
            self.assertIsNotNone(row)
            self.assertIsNone(row["transaction_id"])

    def test_bind_resource_lease_to_transaction_rollback_restores_null_transaction(self):
        lease_id = self._record_lease_id(f"TEST-LEASE-BIND-RB-{uuid.uuid4().hex[:8]}")
        resource_kind = "PORT"
        resource_id = self._existing_port_id()
        lease_owner = "OWNER-BIND-RB"
        lease_token = f"TOKEN-BIND-RB-{uuid.uuid4().hex[:8]}"
        expires_at = "2099-12-31T23:59:59Z"
        transaction_id = f"TEST-TX-BIND-RB-{uuid.uuid4().hex[:8]}"

        self.store.acquire_resource_lease(
            lease_id,
            resource_kind,
            resource_id,
            lease_owner,
            lease_token,
            expires_at,
            transaction_id=None,
        )

        conn = self.store._connection()
        with self.assertRaises(RuntimeError):
            with conn.transaction():
                conn.execute(
                    "INSERT INTO transactions (transaction_id, idempotency_key, correlation_id, port_id, status, validated_by) VALUES (%s, %s, %s, %s, %s, %s);",
                    (transaction_id, transaction_id, transaction_id, resource_id, "RESERVED", "test"),
                )
                result = self.store.bind_resource_lease_to_transaction(lease_id, lease_token, transaction_id)
                self.assertTrue(result)
                raise RuntimeError("force rollback")

        with self._new_connection() as other:
            with other.cursor() as cur:
                cur.execute("SELECT transaction_id FROM resource_leases WHERE lease_id = %s;", (lease_id,))
                row = cur.fetchone()
                self.assertIsNotNone(row)
                self.assertIsNone(row["transaction_id"])
                cur.execute("SELECT transaction_id FROM transactions WHERE transaction_id = %s;", (transaction_id,))
                tx_row = cur.fetchone()
            self.assertIsNone(tx_row)

    def test_release_resource_lease_twice(self):
        lease_id = self._record_lease_id(f"TEST-LEASE-REL-TWICE-{uuid.uuid4().hex[:8]}")
        resource_kind = "PORT"
        resource_id = f"TEST-RESOURCE-LEASE-REL-TWICE-{uuid.uuid4().hex[:8]}"
        lease_owner = "OWNER-REL-TWICE"
        lease_token = f"TOKEN-REL-TWICE-{uuid.uuid4().hex[:8]}"
        expires_at = "2099-12-31T23:59:59Z"

        conn = self.store._connection()
        with conn.transaction() as tx:
            self.store.acquire_resource_lease(
                lease_id,
                resource_kind,
                resource_id,
                lease_owner,
                lease_token,
                expires_at,
                transaction_id=None,
            )
            self.assertTrue(self.store.release_resource_lease(lease_id, lease_token))
            self.assertFalse(self.store.release_resource_lease(lease_id, lease_token))
            raise psycopg.Rollback(tx)

    def test_release_allows_new_lease_after_release(self):
        original_lease_id = self._record_lease_id(f"TEST-LEASE-REL-NEW-{uuid.uuid4().hex[:8]}")
        resource_kind = "PORT"
        resource_id = f"TEST-RESOURCE-LEASE-REL-NEW-{uuid.uuid4().hex[:8]}"
        lease_owner = "OWNER-REL-NEW"
        lease_token = f"TOKEN-REL-NEW-{uuid.uuid4().hex[:8]}"
        expires_at = "2099-12-31T23:59:59Z"

        conn = self.store._connection()
        with conn.transaction() as tx:
            self.store.acquire_resource_lease(
                original_lease_id,
                resource_kind,
                resource_id,
                lease_owner,
                lease_token,
                expires_at,
                transaction_id=None,
            )
            self.assertTrue(self.store.release_resource_lease(original_lease_id, lease_token))
            new_lease_id = self._record_lease_id(f"TEST-LEASE-REL-NEW2-{uuid.uuid4().hex[:8]}")
            new_lease_token = f"TOKEN-REL-NEW2-{uuid.uuid4().hex[:8]}"
            self.store.acquire_resource_lease(
                new_lease_id,
                resource_kind,
                resource_id,
                lease_owner,
                new_lease_token,
                expires_at,
                transaction_id=None,
            )
            with conn.cursor() as cur:
                cur.execute("SELECT count(*) FROM resource_leases WHERE resource_id = %s AND released_at IS NULL;", (resource_id,))
                row = cur.fetchone()
            self.assertEqual(row["count"], 1)
            raise psycopg.Rollback(tx)

    def test_release_resource_lease_returns_false_if_unknown_or_already_released(self):
        lease_id = f"TEST-LEASE-MISSING-{uuid.uuid4().hex[:8]}"
        self.assertFalse(self.store.release_resource_lease(lease_id, "ANY-TOKEN"))


if __name__ == "__main__":
    unittest.main(verbosity=2)
