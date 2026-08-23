import multiprocessing
import tempfile
import unittest
from pathlib import Path

from engine.resource_engine import InvalidTransitionError, ResourceRegistry, ResourceState, TransactionStatus


def reserve_in_process(state_path, technician_id, result_queue):
    registry = ResourceRegistry(state_path)
    try:
        transaction = registry.reserve_resource("PORT-01", technician_id)
        result_queue.put((technician_id, "RESERVED", transaction["transaction_id"]))
    except InvalidTransitionError:
        result_queue.put((technician_id, "REJECTED", None))


class ResourceEngineTests(unittest.TestCase):
    def setUp(self):
        self.registry = ResourceRegistry()
        self.registry.register_resource("PORT-01", "PORT", cabinet_id="CAB-A", line_id="FTTH-LINE-A")

    def reserve(self, technician_id):
        return self.registry.reserve_resource("PORT-01", technician_id)

    def test_free_to_reserved(self):
        transaction = self.reserve("TECH-001")
        self.assertEqual(transaction["new_state"], "RESERVED")
        self.assertEqual(transaction["status"], TransactionStatus.RESERVED.value)
        self.assertEqual(self.registry.get_resource_state("PORT-01"), ResourceState.RESERVED)

    def test_double_reservation_rejected(self):
        self.reserve("TECH-001")
        with self.assertRaises(InvalidTransitionError):
            self.reserve("TECH-002")

    def test_reserved_to_connected(self):
        transaction = self.reserve("TECH-001")
        result = self.registry.connect_resource(transaction["transaction_id"], "TECH-001")
        self.assertEqual(result["status"], TransactionStatus.CONNECTED.value)
        self.assertEqual(self.registry.get_resource_state("PORT-01"), ResourceState.CONNECTED)

    def test_connected_to_verified(self):
        transaction = self.reserve("TECH-001")
        self.registry.connect_resource(transaction["transaction_id"], "TECH-001")
        result = self.registry.verify_resource(transaction["transaction_id"], "TECH-001")
        self.assertEqual(result["status"], TransactionStatus.VERIFIED.value)
        self.assertEqual(self.registry.get_resource_state("PORT-01"), ResourceState.VERIFIED)

    def test_verified_to_occupied(self):
        transaction = self.reserve("TECH-001")
        self.registry.connect_resource(transaction["transaction_id"], "TECH-001")
        self.registry.verify_resource(transaction["transaction_id"], "TECH-001")
        result = self.registry.commit_resource(transaction["transaction_id"], "TECH-001")
        self.assertEqual(result["status"], TransactionStatus.COMMITTED.value)
        self.assertEqual(self.registry.get_resource_state("PORT-01"), ResourceState.OCCUPIED)

    def test_rollback_restores_free(self):
        transaction = self.reserve("TECH-001")
        self.registry.connect_resource(transaction["transaction_id"], "TECH-001")
        result = self.registry.rollback_transaction(transaction["transaction_id"], "TECH-001")
        self.assertEqual(result["status"], TransactionStatus.ROLLED_BACK.value)
        self.assertEqual(self.registry.get_resource_state("PORT-01"), ResourceState.FREE)
        self.assertGreaterEqual(len(self.registry.audit_log()), 3)

    def test_illegal_transition_rejected(self):
        with self.assertRaises(InvalidTransitionError):
            self.registry.connect_resource("missing", "TECH-001")
        transaction = self.reserve("TECH-001")
        with self.assertRaises(InvalidTransitionError):
            self.registry.commit_resource(transaction["transaction_id"], "TECH-001")

    def test_audit_history_preserved(self):
        transaction = self.reserve("TECH-001")
        self.registry.connect_resource(transaction["transaction_id"], "TECH-001")
        history = self.registry.audit_log()
        self.assertEqual(history[0]["previous_state"], "FREE")
        self.assertEqual(history[0]["new_state"], "RESERVED")
        self.assertEqual(history[1]["previous_state"], "RESERVED")
        self.assertEqual(history[1]["new_state"], "CONNECTED")

    def test_persistence_across_restart_and_commit(self):
        with tempfile.TemporaryDirectory() as directory:
            state_path = Path(directory) / "runtime-state.json"
            first = ResourceRegistry(state_path)
            first.register_resource("PORT-01", "PORT")
            transaction = first.reserve_resource("PORT-01", "TECH-TEST")
            first.connect_resource(transaction["transaction_id"], "TECH-TEST")
            restarted = ResourceRegistry(state_path)
            self.assertEqual(restarted.get_resource_state("PORT-01"), ResourceState.CONNECTED)
            self.assertEqual(restarted.transaction(transaction["transaction_id"])["status"], TransactionStatus.CONNECTED.value)
            self.assertEqual(len(restarted.audit_log()), 2)
            restarted.verify_resource(transaction["transaction_id"], "TECH-TEST")
            restarted.commit_resource(transaction["transaction_id"], "TECH-TEST")
            self.assertEqual(ResourceRegistry(state_path).get_resource_state("PORT-01"), ResourceState.OCCUPIED)

    def test_unregister_resource_removes_from_state_and_persists(self):
        with tempfile.TemporaryDirectory() as directory:
            state_path = Path(directory) / "runtime-state.json"
            registry = ResourceRegistry(state_path)
            registry.register_resource("PORT-TEST", "PORT")
            registry.unregister_resource("PORT-TEST")
            self.assertNotIn("PORT-TEST", registry._resources)
            self.assertEqual(ResourceRegistry(state_path).load_state() or None, None)
            restarted = ResourceRegistry(state_path)
            with self.assertRaises(KeyError):
                restarted.get_resource("PORT-TEST")

    def test_unregister_resource_missing_raises(self):
        registry = ResourceRegistry()
        with self.assertRaises(KeyError):
            registry.unregister_resource("MISSING-PORT")

    def test_unregister_resource_restores_memory_on_save_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            state_path = Path(directory) / "runtime-state.json"
            registry = ResourceRegistry(state_path)
            registry.register_resource("PORT-TEST", "PORT")
            original_save = registry._save_locked

            def failing_save():
                raise IOError("save failed")

            registry._save_locked = failing_save
            with self.assertRaises(IOError):
                registry.unregister_resource("PORT-TEST")
            self.assertIn("PORT-TEST", registry._resources)
            registry._save_locked = original_save

    def test_register_resource_rollback_on_save_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            state_path = Path(directory) / "runtime-state.json"
            registry = ResourceRegistry(state_path)
            original_save = registry._save_locked

            def failing_save():
                raise IOError("save failed")

            registry._save_locked = failing_save
            with self.assertRaises(IOError):
                registry.register_resource("PORT-FAIL", "PORT")
            self.assertNotIn("PORT-FAIL", registry._resources)
            registry._save_locked = original_save

    def test_reservation_survives_restart(self):
        with tempfile.TemporaryDirectory() as directory:
            state_path = Path(directory) / "runtime-state.json"
            first = ResourceRegistry(state_path)
            first.register_resource("PORT-01", "PORT")
            transaction = first.reserve_resource("PORT-01", "TECH-TEST")
            restarted = ResourceRegistry(state_path)
            self.assertEqual(restarted.get_resource_state("PORT-01"), ResourceState.RESERVED)
            self.assertEqual(restarted.active_reservations()["PORT-01"], transaction["transaction_id"])
            with self.assertRaises(InvalidTransitionError):
                restarted.reserve_resource("PORT-01", "TECH-OTHER")

    def test_rollback_persists_after_restart(self):
        with tempfile.TemporaryDirectory() as directory:
            state_path = Path(directory) / "runtime-state.json"
            first = ResourceRegistry(state_path)
            first.register_resource("PORT-01", "PORT")
            transaction = first.reserve_resource("PORT-01", "TECH-TEST")
            first.connect_resource(transaction["transaction_id"], "TECH-TEST")
            first.rollback_transaction(transaction["transaction_id"], "TECH-TEST")
            restarted = ResourceRegistry(state_path)
            self.assertEqual(restarted.get_resource_state("PORT-01"), ResourceState.FREE)
            self.assertEqual(restarted.transaction(transaction["transaction_id"])["status"], TransactionStatus.ROLLED_BACK.value)
            self.assertEqual(restarted.audit_log()[-1]["new_state"], ResourceState.FREE.value)

    def test_two_process_double_reservation(self):
        with tempfile.TemporaryDirectory() as directory:
            state_path = str(Path(directory) / "runtime-state.json")
            ResourceRegistry(state_path).register_resource("PORT-01", "PORT")
            result_queue = multiprocessing.Queue()
            processes = [multiprocessing.Process(target=reserve_in_process, args=(state_path, technician, result_queue)) for technician in ("TECH-001", "TECH-002")]
            for process in processes:
                process.start()
            results = [result_queue.get(timeout=10) for _ in processes]
            for process in processes:
                process.join(timeout=10)
            self.assertEqual(sorted(result[1] for result in results), ["REJECTED", "RESERVED"])
            self.assertEqual(ResourceRegistry(state_path).get_resource_state("PORT-01"), ResourceState.RESERVED)

    def test_corrupted_state_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            state_path = Path(directory) / "runtime-state.json"
            state_path.write_text("{not-json", encoding="utf-8")
            with self.assertRaises(ValueError):
                ResourceRegistry(state_path)

    def test_state_models_are_separate(self):
        self.assertNotIn("ROLLED_BACK", [state.value for state in ResourceState])
        self.assertIn("ROLLED_BACK", [status.value for status in TransactionStatus])


if __name__ == "__main__":
    unittest.main(verbosity=2)
