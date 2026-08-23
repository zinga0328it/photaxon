import os
import unittest
import uuid

from engine.storage_backend import create_registry, resolve_backend_name


class BackendSelectionTests(unittest.TestCase):
    def setUp(self):
        self._original = os.environ.get("FIBERSPIDER_STORAGE_BACKEND")

    def tearDown(self):
        if self._original is None:
            os.environ.pop("FIBERSPIDER_STORAGE_BACKEND", None)
        else:
            os.environ["FIBERSPIDER_STORAGE_BACKEND"] = self._original

    def test_default_backend_is_json(self):
        os.environ.pop("FIBERSPIDER_STORAGE_BACKEND", None)
        self.assertEqual(resolve_backend_name(), "json")

    def test_explicit_json_backend(self):
        os.environ["FIBERSPIDER_STORAGE_BACKEND"] = "json"
        self.assertEqual(resolve_backend_name(), "json")

    def test_unknown_backend_raises(self):
        os.environ["FIBERSPIDER_STORAGE_BACKEND"] = "mongodb"
        with self.assertRaises(ValueError):
            resolve_backend_name()

    def test_create_registry_defaults_to_json_in_memory(self):
        os.environ.pop("FIBERSPIDER_STORAGE_BACKEND", None)
        registry = create_registry()
        registry.register_resource("PORT-BACKEND-TEST", "PORT")
        transaction = registry.reserve_resource("PORT-BACKEND-TEST", "TECH-001")
        self.assertEqual(transaction["new_state"], "RESERVED")

    def test_postgres_backend_unreachable_fails_loud(self):
        original_env = {key: os.environ.get(key) for key in (
            "FIBERSPIDER_STORAGE_BACKEND",
            "FIBERSPIDER_DB_HOST",
            "FIBERSPIDER_DB_PORT",
            "FIBERSPIDER_DB_NAME",
            "FIBERSPIDER_DB_USER",
            "FIBERSPIDER_DB_PASSWORD",
        )}
        os.environ["FIBERSPIDER_STORAGE_BACKEND"] = "postgres"
        os.environ["FIBERSPIDER_DB_HOST"] = "::1"
        os.environ["FIBERSPIDER_DB_PORT"] = "5499"
        os.environ["FIBERSPIDER_DB_NAME"] = "fiberspider"
        os.environ["FIBERSPIDER_DB_USER"] = "fiberspider_app"
        os.environ["FIBERSPIDER_DB_PASSWORD"] = "unreachable"
        try:
            with self.assertRaises(Exception):
                create_registry()
        finally:
            for key, value in original_env.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value


@unittest.skipUnless(os.environ.get("FIBERSPIDER_RUN_DB_TESTS") == "1", "requires FIBERSPIDER_RUN_DB_TESTS=1")
class PostgresBackendParityTests(unittest.TestCase):
    """Runs the same FREE->RESERVED->CONNECTED->VERIFIED->OCCUPIED sequence against the
    postgres backend, inside one uncommitted transaction, then rolls everything back."""

    def setUp(self):
        os.environ["FIBERSPIDER_STORAGE_BACKEND"] = "postgres"
        self.registry = create_registry()
        self.port_id = f"TEST-PORT-PARITY-{uuid.uuid4().hex[:8]}"

    def tearDown(self):
        self.registry.store.connection().rollback()
        self.registry.store.close()
        os.environ.pop("FIBERSPIDER_STORAGE_BACKEND", None)

    def test_full_sequence_matches_json_backend_rules(self):
        self.registry.register_resource(self.port_id, "PORT", splitter_id="SPL-A-01")
        transaction = self.registry.reserve_resource(self.port_id, "TECH-001")
        self.assertEqual(transaction["new_state"], "RESERVED")
        transaction = self.registry.connect_resource(transaction["transaction_id"], "TECH-001")
        self.assertEqual(transaction["new_state"], "CONNECTED")
        transaction = self.registry.verify_resource(transaction["transaction_id"], "TECH-001")
        self.assertEqual(transaction["new_state"], "VERIFIED")
        transaction = self.registry.commit_resource(transaction["transaction_id"], "TECH-001")
        self.assertEqual(transaction["new_state"], "OCCUPIED")

        conn = self.registry.store.connection()
        with conn.cursor() as cur:
            cur.execute("SELECT resource_state FROM ports WHERE port_id = %s;", (self.port_id,))
            row = cur.fetchone()
        self.assertEqual(row["resource_state"], "OCCUPIED")

        with conn.cursor() as cur:
            cur.execute("SELECT resource_state FROM resource_states WHERE resource_id = %s;", (self.port_id,))
            state_row = cur.fetchone()
        self.assertEqual(state_row["resource_state"], "OCCUPIED")

    def test_resource_states_reflect_full_sequence(self):
        self.registry.register_resource(self.port_id, "PORT", splitter_id="SPL-A-01")
        conn = self.registry.store.connection()
        with conn.cursor() as cur:
            cur.execute("SELECT resource_state FROM resource_states WHERE resource_id = %s;", (self.port_id,))
            self.assertEqual(cur.fetchone()["resource_state"], "FREE")

        transaction = self.registry.reserve_resource(self.port_id, "TECH-001")
        with conn.cursor() as cur:
            cur.execute("SELECT resource_state FROM resource_states WHERE resource_id = %s;", (self.port_id,))
            self.assertEqual(cur.fetchone()["resource_state"], "RESERVED")

        transaction = self.registry.connect_resource(transaction["transaction_id"], "TECH-001")
        with conn.cursor() as cur:
            cur.execute("SELECT resource_state FROM resource_states WHERE resource_id = %s;", (self.port_id,))
            self.assertEqual(cur.fetchone()["resource_state"], "CONNECTED")

        transaction = self.registry.verify_resource(transaction["transaction_id"], "TECH-001")
        with conn.cursor() as cur:
            cur.execute("SELECT resource_state FROM resource_states WHERE resource_id = %s;", (self.port_id,))
            self.assertEqual(cur.fetchone()["resource_state"], "VERIFIED")

        transaction = self.registry.commit_resource(transaction["transaction_id"], "TECH-001")
        with conn.cursor() as cur:
            cur.execute("SELECT resource_state FROM resource_states WHERE resource_id = %s;", (self.port_id,))
            self.assertEqual(cur.fetchone()["resource_state"], "OCCUPIED")

    def test_sync_rolls_back_on_error_after_transaction_insert(self):
        from engine.resource_engine import InvalidTransitionError

        self.registry.register_resource(self.port_id, "PORT", splitter_id="SPL-A-01")
        conn = self.registry.store.connection()
        with conn.cursor() as cur:
            cur.execute("SELECT resource_state FROM resource_states WHERE resource_id = %s;", (self.port_id,))
            self.assertEqual(cur.fetchone()["resource_state"], "FREE")
            cur.execute("SELECT resource_state FROM ports WHERE port_id = %s;", (self.port_id,))
            self.assertEqual(cur.fetchone()["resource_state"], "FREE")

        original_update = self.registry.store.update_resource_state

        def fail_update(*args, **kwargs):
            raise RuntimeError("forced sync failure")

        self.registry.store.update_resource_state = fail_update
        with self.assertRaises(RuntimeError):
            self.registry.reserve_resource(self.port_id, "TECH-001")
        self.registry.store.update_resource_state = original_update

        with conn.cursor() as cur:
            cur.execute("SELECT transaction_id FROM transactions WHERE port_id = %s;", (self.port_id,))
            self.assertIsNone(cur.fetchone())
            cur.execute("SELECT resource_state FROM resource_states WHERE resource_id = %s;", (self.port_id,))
            row = cur.fetchone()
            self.assertIsNotNone(row)
            self.assertEqual(row["resource_state"], "FREE")
            cur.execute("SELECT resource_state FROM ports WHERE port_id = %s;", (self.port_id,))
            row = cur.fetchone()
            self.assertIsNotNone(row)
            self.assertEqual(row["resource_state"], "FREE")

    def test_illegal_transition_rejected_same_as_json_backend(self):
        from engine.resource_engine import InvalidTransitionError

        self.registry.register_resource(self.port_id, "PORT", splitter_id="SPL-A-01")
        transaction = self.registry.reserve_resource(self.port_id, "TECH-001")
        with self.assertRaises(InvalidTransitionError):
            self.registry.commit_resource(transaction["transaction_id"], "TECH-001")

    def test_no_permanent_data_after_rollback(self):
        conn = self.registry.store.connection()
        try:
            with conn.transaction():
                self.registry.register_resource(self.port_id, "PORT", splitter_id="SPL-A-01")
                self.registry.reserve_resource(self.port_id, "TECH-001")
                with conn.cursor() as cur:
                    cur.execute("SELECT port_id FROM ports WHERE port_id = %s;", (self.port_id,))
                    self.assertIsNotNone(cur.fetchone())
                raise RuntimeError("rollback now")
        except RuntimeError:
            pass

        with conn.cursor() as cur:
            cur.execute("SELECT port_id FROM ports WHERE port_id = %s;", (self.port_id,))
            self.assertIsNone(cur.fetchone())
            cur.execute("SELECT resource_id FROM resource_states WHERE resource_id = %s;", (self.port_id,))
            self.assertIsNone(cur.fetchone())

    def test_register_resource_atomic_with_local_compensation(self):
        # use a nonexistent splitter_id to trigger DB failure on ports insert
        bad_port = f"TEST-PORT-REG-FAIL-{uuid.uuid4().hex[:8]}"
        with self.assertRaises(Exception):
            self.registry.register_resource(bad_port, "PORT", splitter_id="SPL-UNKNOWN")

        conn = self.registry.store.connection()
        with conn.cursor() as cur:
            cur.execute("SELECT port_id FROM ports WHERE port_id = %s;", (bad_port,))
            self.assertIsNone(cur.fetchone())
            cur.execute("SELECT resource_id FROM resource_states WHERE resource_id = %s;", (bad_port,))
            self.assertIsNone(cur.fetchone())
        self.assertNotIn(bad_port, self.registry.registry._resources)

    def test_register_resource_raises_exceptiongroup_when_compensation_fails(self):
        bad_port = f"TEST-PORT-REG-FAIL-{uuid.uuid4().hex[:8]}"
        original_create = self.registry.store.create_resource_state
        original_unregister = self.registry.unregister_resource

        def fail_create_resource_state(*args, **kwargs):
            raise RuntimeError("forced db failure")

        def fail_unregister_resource(resource_id):
            raise RuntimeError("forced unregister failure")

        self.registry.store.create_resource_state = fail_create_resource_state
        self.registry.registry.unregister_resource = fail_unregister_resource

        try:
            with self.assertRaises(ExceptionGroup) as cm:
                self.registry.register_resource(bad_port, "PORT", splitter_id="SPL-A-01")
        finally:
            self.registry.store.create_resource_state = original_create
            self.registry.registry.unregister_resource = original_unregister

        self.assertEqual(len(cm.exception.exceptions), 2)
        self.assertTrue(any(isinstance(exc, RuntimeError) and str(exc) == "forced db failure" for exc in cm.exception.exceptions))
        self.assertTrue(any(isinstance(exc, RuntimeError) and str(exc) == "forced unregister failure" for exc in cm.exception.exceptions))

    def test_register_resource_success_commits_and_is_visible(self):
        good_port = f"TEST-PORT-REG-SUCCESS-{uuid.uuid4().hex[:8]}"
        self.registry.register_resource(good_port, "PORT", splitter_id="SPL-A-01")

        conn = self.registry.store.connection()
        self.assertEqual(conn.info.transaction_status, 0)
        with conn.cursor() as cur:
            cur.execute("SELECT resource_state FROM resource_states WHERE resource_id = %s;", (good_port,))
            row = cur.fetchone()
        self.assertIsNotNone(row)
        self.assertEqual(row["resource_state"], "FREE")
        with conn.cursor() as cur:
            cur.execute("SELECT resource_state FROM ports WHERE port_id = %s;", (good_port,))
            row = cur.fetchone()
        self.assertIsNotNone(row)
        self.assertEqual(row["resource_state"], "FREE")

        import psycopg
        with psycopg.connect(
            host=os.environ["FIBERSPIDER_DB_HOST"],
            port=os.environ["FIBERSPIDER_DB_PORT"],
            dbname=os.environ["FIBERSPIDER_DB_NAME"],
            user=os.environ["FIBERSPIDER_DB_USER"],
            password=os.environ["FIBERSPIDER_DB_PASSWORD"],
            row_factory=psycopg.rows.dict_row,
        ) as conn2:
            with conn2.cursor() as cur2:
                cur2.execute("SELECT resource_state FROM resource_states WHERE resource_id = %s;", (good_port,))
                row2 = cur2.fetchone()
                self.assertIsNotNone(row2)
                self.assertEqual(row2["resource_state"], "FREE")
                cur2.execute("SELECT resource_state FROM ports WHERE port_id = %s;", (good_port,))
                row2 = cur2.fetchone()
                self.assertIsNotNone(row2)
                self.assertEqual(row2["resource_state"], "FREE")


if __name__ == "__main__":
    unittest.main(verbosity=2)
