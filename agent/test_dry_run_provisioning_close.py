from __future__ import annotations

TECHNICIAN = "1234567890"
TECHNICIAN_AUTHENTICATED = "YES"
FIELD_PATH_CONFIRMED = "YES"
ONT_SERIAL = "FS-ONT-DEMO-0001"
GPON_PRESENT = "YES"
PROVISIONING_PLAN_CREATED = "YES"
NEW_ROE_REGISTRATION_PLANNED = "YES"
ONT_ASSOCIATION_PLANNED = "YES"
PROVISIONING_STATUS = "READY"
ONT_PROVISIONING = "SUCCESS"
CUSTOMER_LINE = "ACTIVE"
FIELD_ACTIVITY = "CLOSED"
FINAL_STATE = "JOB_CLOSED"
REAL_PROVISIONING_EXECUTED = "NO"
LEASE_ACQUIRED = "NO"
DB_MODIFIED = "NO"
OPENSTACK_MODIFIED = "NO"

PROPOSED_PATH = (
    "CENTRAL-01 → CAB-A → FTTH-LINE-A → SPL-A-01 → ROE-LAB-001 → FS-ONT-DEMO-0001"
)

PROVISIONING_PLAN = {
    "register_new_roe": {
        "resource_id": "ROE-LAB-001",
        "parent_splitter": "SPL-A-01",
        "cabinet": "CAB-A",
        "line": "FTTH-LINE-A",
    },
    "associate_ont": {
        "ont_serial": ONT_SERIAL,
        "roe": "ROE-LAB-001",
        "status": "prepared",
    },
    "prepare_provisioning": {
        "action": "prepare_ont_provisioning",
        "details": "ONT provisioning plan created in memory",
    },
    "activate_customer_service": {
        "status": "ready",
        "customer_line": "ACTIVE",
    },
    "finalize_topology": {
        "new_resource": "ROE-LAB-001",
        "ont": ONT_SERIAL,
        "path_confirmed": True,
    },
    "close_field_activity": {
        "status": "CLOSED",
        "technician_id": TECHNICIAN,
    },
}


def main() -> None:
    report = {
        "TECHNICIAN AUTHENTICATED": TECHNICIAN_AUTHENTICATED,
        "FIELD PATH CONFIRMED": FIELD_PATH_CONFIRMED,
        "ONT": ONT_SERIAL,
        "GPON PRESENT": GPON_PRESENT,
        "PROVISIONING PLAN CREATED": PROVISIONING_PLAN_CREATED,
        "NEW ROE REGISTRATION PLANNED": NEW_ROE_REGISTRATION_PLANNED,
        "ONT ASSOCIATION PLANNED": ONT_ASSOCIATION_PLANNED,
        "PROVISIONING STATUS": PROVISIONING_STATUS,
        "ONT PROVISIONING": ONT_PROVISIONING,
        "CUSTOMER LINE": CUSTOMER_LINE,
        "FIELD ACTIVITY": FIELD_ACTIVITY,
        "FINAL STATE": FINAL_STATE,
        "REAL PROVISIONING EXECUTED": REAL_PROVISIONING_EXECUTED,
        "LEASE ACQUIRED": LEASE_ACQUIRED,
        "DB MODIFIED": DB_MODIFIED,
        "OPENSTACK MODIFIED": OPENSTACK_MODIFIED,
        "PROPOSED PATH": PROPOSED_PATH,
    }

    for key, value in report.items():
        print(f"{key}: {value}")


if __name__ == "__main__":
    main()
