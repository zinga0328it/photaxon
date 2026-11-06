# ZINGA Broker Integration (Future)

## Roles and Trust Boundaries

ZINGA is a separate PC/server, not a RAGNO library. Its future role is broker and key manager: service identity, key issuance/rotation/revocation/expiry, credential distribution, client authentication metadata, broker events, correlation IDs, rotation audit, replay protection, event deduplication and node heartbeats.

```text
ALEX -> RAGNO FiberSpider API (dashboard, read-only)
ZINGA server -> Yggdrasil -> RAGNO API/MCP -> Rust Validator -> AAA PostgreSQL
CODEX -> authorized Rust MCP tools -> Validator -> domain services -> AAA
```

ZINGA must not connect directly to AAA or receive arbitrary SQL, shell, Linux administration, OpenStack write credentials, MCP admin access or RAGNO secrets. RAGNO remains the FTTH control plane and domain gatekeeper. AAA remains the persistent database. CODEX remains reasoning via authorized MCP, not an OS administrator.

## Future Identity

The future `zinga-broker` identity may receive a separately tested `broker` scope. It is not configured now. It does not imply `dashboard`, `admin`, `shell`, `db`, `mcp:write` or secret access. Every broker operation must map to an explicit API/MCP action, validation rule and audit record.

The Rust `CredentialProvider` boundary currently has an environment-backed implementation. A future provider can read versioned identity/key metadata from ZINGA. During ZINGA outage, RAGNO may use only already-issued valid keys per an explicit cache policy; it must never mint credentials locally, bypass authentication or enter anonymous mode. New authorization is DENY/DEFER when the broker cannot establish it.

## Versioned Event Contract

Example only; payload contains no secret:

```json
{
  "schema_version": "1",
  "event_id": "<EVENT_ID>",
  "correlation_id": "<CORRELATION_ID>",
  "idempotency_key": "<IDEMPOTENCY_KEY>",
  "source": "zinga-broker",
  "target": "ragno",
  "event_type": "<EVENT_TYPE>",
  "timestamp": "<RFC3339_UTC>",
  "key_id": "<KEY_ID>",
  "payload": {}
}
```

The transport and schema require authenticated Yggdrasil peer identity and a versioned secure API/MCP contract. RAGNO validates schema version, source, target, scope, timestamp skew, event ID, idempotency key and payload before domain processing. Reject reused event/idempotency IDs, stale timestamps, unknown key IDs, revoked/expired keys and unsupported schema versions.

## Delivery Policy

- Use a finite connect/read timeout; proposed starting limits are 3 s connect and 10 s response, tuned by tests before activation.
- Retry only transient transport/5xx failures with bounded exponential backoff and the same idempotency key. Do not retry 400/401/403/409/422 unchanged.
- Errors: 400 invalid schema, 401 unauthenticated/unknown key, 403 scope denied, 409 duplicate/replay, 422 domain validation rejected, 503 broker/provider unavailable, 504 timeout.
- Persist event ID, correlation ID, key ID, event type, timestamp and result in an audit trail. Never log token/secret/payload credentials.
- Heartbeats carry node ID, protocol/schema version and timestamp only; they do not prove domain health or grant authorization.

No event bus or ZINGA client is implemented in this phase. Add a protocol only after its threat model, versioning, replay/idempotency tests, audit and rollback plan are reviewed.
