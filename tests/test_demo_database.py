import os
import unittest
from demo.run_demo import connect, run

@unittest.skipUnless(os.environ.get("FIBERSPIDER_RUN_DEMO_DB_TEST") == "1", "requires demo PostgreSQL")
class DemoDatabaseIntegrationTests(unittest.TestCase):
    def test_demo_persists_event_validation_and_reservation(self):
        with connect() as conn:
            conn.execute("UPDATE demo_resources SET state = 'FREE', owner_transaction_id = NULL WHERE resource_id = 'PORT-A-01'")
            conn.commit()
        result = run()
        with connect() as conn:
            run_row = conn.execute("SELECT final_state FROM demo_runs WHERE run_id = %s", (result["postgresql"]["run_id"],)).fetchone()
            resource = conn.execute("SELECT state FROM demo_resources WHERE resource_id = 'PORT-A-01'").fetchone()
        self.assertEqual(run_row["final_state"], "RESOURCE_RESERVED")
        self.assertEqual(resource["state"], "RESERVED")

if __name__ == "__main__": unittest.main()

