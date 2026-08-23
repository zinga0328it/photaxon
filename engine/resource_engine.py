from __future__ import annotations

import fcntl
import json
import os
import tempfile
from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum
from threading import RLock
from typing import Any
from uuid import uuid4


class ResourceState(str, Enum):
    FREE = "FREE"
    RESERVED = "RESERVED"
    CONNECTED = "CONNECTED"
    VERIFIED = "VERIFIED"
    OCCUPIED = "OCCUPIED"
    FAULT = "FAULT"


class TransactionStatus(str, Enum):
    RESERVED = "RESERVED"
    CONNECTED = "CONNECTED"
    VERIFIED = "VERIFIED"
    COMMITTED = "COMMITTED"
    REJECTED = "REJECTED"
    ROLLED_BACK = "ROLLED_BACK"


_ALLOWED_TRANSITIONS = {
    ResourceState.FREE: {ResourceState.RESERVED},
    ResourceState.RESERVED: {ResourceState.CONNECTED, ResourceState.FREE},
    ResourceState.CONNECTED: {ResourceState.VERIFIED},
    ResourceState.VERIFIED: {ResourceState.OCCUPIED},
    ResourceState.OCCUPIED: set(),
    ResourceState.FAULT: set(),
}


class InvalidTransitionError(ValueError):
    pass


@dataclass
class Resource:
    resource_id: str
    resource_type: str
    state: ResourceState = ResourceState.FREE
    owner_transaction_id: str | None = None
    metadata: dict[str, Any] = field(default_factory=dict)


class ResourceRegistry:
    def __init__(self, state_path: str | os.PathLike[str] | None = None, lock_path: str | os.PathLike[str] | None = None) -> None:
        self.state_path = os.fspath(state_path) if state_path else None
        self.lock_path = os.fspath(lock_path or f"{self.state_path}.lock") if self.state_path else None
        self._resources: dict[str, Resource] = {}
        self._transactions: dict[str, dict[str, Any]] = {}
        self._audit_log: list[dict[str, Any]] = []
        self._active_reservations: dict[str, str] = {}
        self._lock = RLock()
        if self.state_path and os.path.exists(self.state_path):
            self.load_state()

    def register_resource(self, resource_id: str, resource_type: str, state: ResourceState = ResourceState.FREE, **metadata: Any) -> Resource:
        with self._locked_state():
            self._reload_locked()
            if resource_id in self._resources:
                raise ValueError(f"resource already registered: {resource_id}")
            resource = Resource(resource_id, resource_type, state, metadata=metadata)
            self._resources[resource_id] = resource
            try:
                self._save_locked()
            except Exception:
                self._resources.pop(resource_id, None)
                raise
            return resource

    def unregister_resource(self, resource_id: str) -> None:
        with self._locked_state():
            self._reload_locked()
            if resource_id not in self._resources:
                raise KeyError(f"unknown resource: {resource_id}")
            removed = self._resources.pop(resource_id)
            try:
                self._save_locked()
            except Exception:
                self._resources[resource_id] = removed
                raise

    def load_state(self) -> None:
        if not self.state_path:
            return
        with self._lock:
            try:
                with open(self.state_path, encoding="utf-8") as handle:
                    data = json.load(handle)
            except (OSError, json.JSONDecodeError) as exc:
                raise ValueError(f"invalid runtime state: {self.state_path}") from exc
            self._load_data(data)

    def save_state(self) -> None:
        if not self.state_path:
            return
        with self._locked_state():
            self._save_locked()

    def get_resource_state(self, resource_id: str) -> ResourceState:
        with self._lock:
            return self._get_resource(resource_id).state

    def get_resource(self, resource_id: str) -> Resource:
        with self._lock:
            return self._get_resource(resource_id)

    def reserve_resource(self, resource_id: str, technician_id: str, **context: Any) -> dict[str, Any]:
        return self._transition(resource_id, ResourceState.RESERVED, TransactionStatus.RESERVED, technician_id, "reserve", **context)

    def connect_resource(self, transaction_id: str, technician_id: str, **context: Any) -> dict[str, Any]:
        return self._transition_transaction(transaction_id, ResourceState.CONNECTED, TransactionStatus.CONNECTED, technician_id, "connect", **context)

    def verify_resource(self, transaction_id: str, technician_id: str, **context: Any) -> dict[str, Any]:
        return self._transition_transaction(transaction_id, ResourceState.VERIFIED, TransactionStatus.VERIFIED, technician_id, "verify", **context)

    def commit_resource(self, transaction_id: str, technician_id: str, **context: Any) -> dict[str, Any]:
        return self._transition_transaction(transaction_id, ResourceState.OCCUPIED, TransactionStatus.COMMITTED, technician_id, "commit", **context)

    def release_resource(self, transaction_id: str, technician_id: str, reason: str = "released", **context: Any) -> dict[str, Any]:
        return self.rollback_transaction(transaction_id, technician_id, reason, **context)

    def rollback_transaction(self, transaction_id: str, technician_id: str, reason: str = "rollback", **context: Any) -> dict[str, Any]:
        with self._locked_state():
            self._reload_locked()
            transaction = self._get_transaction(transaction_id)
            resource = self._get_resource(transaction["resource"])
            if resource.state not in {ResourceState.RESERVED, ResourceState.CONNECTED, ResourceState.VERIFIED}:
                raise InvalidTransitionError(f"{resource.resource_id}: {resource.state.value} cannot be rolled back")
            previous_state = resource.state
            timestamp = datetime.now(timezone.utc).isoformat()
            resource.state = ResourceState.FREE
            resource.owner_transaction_id = None
            self._active_reservations.pop(resource.resource_id, None)
            transaction.update({"technician_id": technician_id, "previous_state": previous_state.value, "new_state": ResourceState.FREE.value, "reason": reason, "validated_by": "deterministic-resource-engine", "timestamp": timestamp, "status": TransactionStatus.ROLLED_BACK.value, **context})
            self._transactions[transaction_id] = transaction
            self._audit_log.append({"transaction_id": transaction_id, "technician_id": technician_id, "resource": resource.resource_id, "previous_state": previous_state.value, "new_state": ResourceState.FREE.value, "timestamp": timestamp, "reason": reason})
            self._save_locked()
            return dict(transaction)

    def transaction(self, transaction_id: str) -> dict[str, Any]:
        with self._lock:
            return dict(self._get_transaction(transaction_id))

    def audit_log(self) -> list[dict[str, Any]]:
        with self._lock:
            return [dict(entry) for entry in self._audit_log]

    def active_reservations(self) -> dict[str, str]:
        with self._lock:
            return dict(self._active_reservations)

    def _transition(self, resource_id: str, new_state: ResourceState, status: TransactionStatus, technician_id: str, reason: str, **context: Any) -> dict[str, Any]:
        with self._locked_state():
            self._reload_locked()
            resource = self._get_resource(resource_id)
            transaction = self._new_transaction(resource, context)
            result = self._transition_locked(resource, new_state, status, technician_id, reason, transaction, **context)
            self._save_locked()
            return result

    def _transition_transaction(self, transaction_id: str, new_state: ResourceState, status: TransactionStatus, technician_id: str, reason: str, **context: Any) -> dict[str, Any]:
        with self._locked_state():
            self._reload_locked()
            transaction = self._get_transaction(transaction_id)
            resource = self._get_resource(transaction["resource"])
            result = self._transition_locked(resource, new_state, status, technician_id, reason, transaction, **context)
            self._save_locked()
            return result

    def _transition_locked(self, resource: Resource, new_state: ResourceState, status: TransactionStatus, technician_id: str, reason: str, transaction: dict[str, Any], **context: Any) -> dict[str, Any]:
        previous_state = resource.state
        if new_state not in _ALLOWED_TRANSITIONS[previous_state]:
            raise InvalidTransitionError(f"{resource.resource_id}: {previous_state.value} -> {new_state.value} is not allowed")
        if new_state == ResourceState.RESERVED and resource.owner_transaction_id:
            raise InvalidTransitionError(f"{resource.resource_id}: already owned by {resource.owner_transaction_id}")
        timestamp = datetime.now(timezone.utc).isoformat()
        resource.state = new_state
        if new_state == ResourceState.RESERVED:
            resource.owner_transaction_id = transaction["transaction_id"]
            self._active_reservations[resource.resource_id] = transaction["transaction_id"]
        elif new_state == ResourceState.FREE:
            resource.owner_transaction_id = None
            self._active_reservations.pop(resource.resource_id, None)
        transaction.update({"technician_id": technician_id, "previous_state": previous_state.value, "new_state": new_state.value, "reason": reason, "validated_by": "deterministic-resource-engine", "timestamp": timestamp, "status": status.value, **context})
        self._transactions[transaction["transaction_id"]] = transaction
        self._audit_log.append({"transaction_id": transaction["transaction_id"], "technician_id": technician_id, "resource": resource.resource_id, "previous_state": previous_state.value, "new_state": new_state.value, "timestamp": timestamp, "reason": reason})
        return dict(transaction)

    def _new_transaction(self, resource: Resource, context: dict[str, Any]) -> dict[str, Any]:
        return {"transaction_id": f"TX-{uuid4().hex[:12].upper()}", "technician_id": context.get("technician_id"), "customer_id": context.get("customer_id", "TEST-CUSTOMER"), "ont_serial": context.get("ont_serial", "TEST-ONT"), "cabinet_id": context.get("cabinet_id"), "line_id": context.get("line_id"), "attachment_point": context.get("attachment_point"), "port": context.get("port", resource.resource_id), "resource": resource.resource_id}

    def _load_data(self, data: dict[str, Any]) -> None:
        if not isinstance(data, dict):
            raise ValueError("runtime state must be an object")
        resources = data.get("resources", {})
        if not isinstance(resources, dict) or not isinstance(data.get("active_reservations", {}), dict) or not isinstance(data.get("transactions", {}), dict) or not isinstance(data.get("transaction_history", []), list):
            raise ValueError("runtime state has invalid collections")
        self._resources = {key: Resource(key, value["resource_type"], ResourceState(value["state"]), value.get("owner_transaction_id"), value.get("metadata", {})) for key, value in resources.items()}
        self._active_reservations = dict(data.get("active_reservations", {}))
        self._transactions = {key: dict(value) for key, value in data.get("transactions", {}).items()}
        self._audit_log = [dict(value) for value in data.get("transaction_history", [])]

    def _reload_locked(self) -> None:
        if self.state_path and os.path.exists(self.state_path):
            with open(self.state_path, encoding="utf-8") as handle:
                self._load_data(json.load(handle))

    def _save_locked(self) -> None:
        if not self.state_path:
            return
        directory = os.path.dirname(os.path.abspath(self.state_path))
        os.makedirs(directory, exist_ok=True)
        data = {"resources": {key: {"resource_type": value.resource_type, "state": value.state.value, "owner_transaction_id": value.owner_transaction_id, "metadata": value.metadata} for key, value in self._resources.items()}, "active_reservations": self._active_reservations, "transactions": self._transactions, "transaction_history": self._audit_log}
        fd, temporary = tempfile.mkstemp(prefix=".runtime-state.", dir=directory, text=True)
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as handle:
                json.dump(data, handle, indent=2, sort_keys=True)
                handle.write("\n")
                handle.flush()
                os.fsync(handle.fileno())
            os.replace(temporary, self.state_path)
        finally:
            if os.path.exists(temporary):
                os.unlink(temporary)

    def _locked_state(self):
        return _StateLock(self)

    def _get_resource(self, resource_id: str) -> Resource:
        try:
            return self._resources[resource_id]
        except KeyError as exc:
            raise KeyError(f"unknown resource: {resource_id}") from exc

    def _get_transaction(self, transaction_id: str) -> dict[str, Any]:
        try:
            return self._transactions[transaction_id]
        except KeyError as exc:
            raise InvalidTransitionError(f"unknown transaction: {transaction_id}") from exc


class _StateLock:
    def __init__(self, registry: ResourceRegistry) -> None:
        self.registry = registry
        self.handle = None

    def __enter__(self):
        self.registry._lock.acquire()
        if self.registry.lock_path:
            directory = os.path.dirname(os.path.abspath(self.registry.lock_path))
            os.makedirs(directory, exist_ok=True)
            self.handle = open(self.registry.lock_path, "a+", encoding="utf-8")
            fcntl.flock(self.handle.fileno(), fcntl.LOCK_EX)
        return self.registry

    def __exit__(self, exc_type, exc_value, traceback):
        if self.handle:
            fcntl.flock(self.handle.fileno(), fcntl.LOCK_UN)
            self.handle.close()
        self.registry._lock.release()
