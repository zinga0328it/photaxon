from __future__ import annotations

import os
from typing import Any

from psycopg.types.json import Json

from engine.postgres_store import PostgresStore
from engine.resource_engine import ResourceRegistry

_VALID_BACKENDS = {"json", "postgres"}


def resolve_backend_name() -> str:
    """Read FIBERSPIDER_STORAGE_BACKEND, defaulting to 'json'. Raises on unknown values."""
    name = os.environ.get("FIBERSPIDER_STORAGE_BACKEND", "json").strip().lower()
    if name not in _VALID_BACKENDS:
        raise ValueError(f"unsupported FIBERSPIDER_STORAGE_BACKEND: {name!r} (expected 'json' or 'postgres')")
    return name


class PostgresBackedRegistry:
    """Persists ResourceRegistry transitions into FiberSpider PostgreSQL tables.

    The state machine is NOT reimplemented here: every mutating call delegates
    to a plain in-memory ResourceRegistry, and the resulting resource/transaction
    state is mirrored into `ports`, `transactions` and `transaction_events`
    afterwards. Only PORT-type resources tied to an existing splitter are
    supported, matching the current normalized schema (no generic blob table).
    """

    def __init__(self, store: PostgresStore) -> None:
        self.store = store
        self.registry = ResourceRegistry()

    def register_resource(self, resource_id: str, resource_type: str, **metadata: Any):
        splitter_id = metadata.get("splitter_id")
        if not splitter_id:
            raise ValueError("postgres backend requires 'splitter_id' metadata to register a port resource")
        conn = self.store.connection()
        resource = None
        local_registered = False
        try:
            with conn.transaction():
                resource = self.registry.register_resource(resource_id, resource_type, **metadata)
                local_registered = True
                # Persist generic resource runtime state in resource_states.
                self.store.create_resource_state(
                    resource_id,
                    resource_type,
                    resource.state.value,
                    owner_transaction_id=None,
                    metadata=resource.metadata,
                )
                # Maintain ports.resource_state explicitly for compatibility with existing queries.
                conn.execute(
                    "INSERT INTO ports (port_id, splitter_id, resource_state) VALUES (%s, %s, %s) "
                    "ON CONFLICT (port_id) DO UPDATE SET resource_state = EXCLUDED.resource_state, updated_at = now();",
                    (resource_id, splitter_id, resource.state.value),
                )
            return resource
        except Exception as original_error:
            if local_registered:
                try:
                    self.registry.unregister_resource(resource_id)
                except Exception as compensation_error:
                    raise ExceptionGroup("postgres register failed and local rollback failed", [original_error, compensation_error]) from original_error
            raise

    def reserve_resource(self, resource_id: str, technician_id: str, **context: Any) -> dict[str, Any]:
        result = self.registry.reserve_resource(resource_id, technician_id, **context)
        try:
            self._sync(result)
        except Exception as sync_error:
            try:
                self.registry.rollback_transaction(result["transaction_id"], technician_id, reason="postgres-sync-failure")
            except Exception as rollback_error:
                raise ExceptionGroup("postgres sync failed and local rollback failed", [sync_error, rollback_error]) from sync_error
            raise
        return result

    def connect_resource(self, transaction_id: str, technician_id: str, **context: Any) -> dict[str, Any]:
        result = self.registry.connect_resource(transaction_id, technician_id, **context)
        self._sync(result)
        return result

    def verify_resource(self, transaction_id: str, technician_id: str, **context: Any) -> dict[str, Any]:
        result = self.registry.verify_resource(transaction_id, technician_id, **context)
        self._sync(result)
        return result

    def commit_resource(self, transaction_id: str, technician_id: str, **context: Any) -> dict[str, Any]:
        result = self.registry.commit_resource(transaction_id, technician_id, **context)
        self._sync(result)
        return result

    def rollback_transaction(self, transaction_id: str, technician_id: str, reason: str = "rollback", **context: Any) -> dict[str, Any]:
        result = self.registry.rollback_transaction(transaction_id, technician_id, reason, **context)
        self._sync(result)
        return result

    def _sync(self, transaction: dict[str, Any]) -> None:
        conn = self.store.connection()
        resource_id = transaction["resource"]
        resource = self.registry.get_resource(resource_id)
        with conn.transaction():
            conn.execute(
                "INSERT INTO transactions (transaction_id, idempotency_key, correlation_id, port_id, status, validated_by) "
                "VALUES (%s, %s, %s, %s, %s, %s) "
                "ON CONFLICT (transaction_id) DO UPDATE SET status = EXCLUDED.status, updated_at = now();",
                (
                    transaction["transaction_id"],
                    transaction["transaction_id"],
                    transaction["transaction_id"],
                    resource_id,
                    transaction["status"],
                    transaction.get("validated_by"),
                ),
            )
            self.store.update_resource_state(
                resource_id,
                transaction["new_state"],
                owner_transaction_id=transaction.get("transaction_id"),
                metadata=resource.metadata,
            )
            conn.execute(
                "UPDATE ports SET resource_state = %s, updated_at = now() WHERE port_id = %s;",
                (transaction["new_state"], resource_id),
            )
            conn.execute(
                "INSERT INTO transaction_events (transaction_id, correlation_id, event_type, previous_state, new_state, payload) "
                "VALUES (%s, %s, %s, %s, %s, %s);",
                (
                    transaction["transaction_id"],
                    transaction["transaction_id"],
                    transaction.get("reason"),
                    transaction.get("previous_state"),
                    transaction["new_state"],
                    Json({}),
                ),
            )

    def __getattr__(self, name: str) -> Any:
        return getattr(self.registry, name)


def create_registry(state_path: str | None = None) -> Any:
    """Factory selecting the storage backend via FIBERSPIDER_STORAGE_BACKEND (default 'json').

    Fails loudly (no silent fallback) if backend='postgres' and PostgreSQL is unreachable.
    """
    backend = resolve_backend_name()
    if backend == "json":
        return ResourceRegistry(state_path)
    store = PostgresStore()
    store.connect()
    return PostgresBackedRegistry(store)
