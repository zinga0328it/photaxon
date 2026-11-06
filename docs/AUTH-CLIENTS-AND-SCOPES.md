# Client Authentication and Scopes

## Current State

RAGNO Rust uses `CredentialProvider` behind the security boundary. The current `EnvCredentialProvider` reads local runtime configuration, stores only token hashes in process memory, and produces a `ClientPrincipal` containing `client_id`, `key_id`, and scopes. FTTH handlers never receive the presented token. API/admin/internal legacy tokens remain separate and unchanged.

`RAGNO MUST NOT hardcode the current local secret backend as a permanent architectural dependency.`

`Credential storage must be replaceable without changing FTTH domain logic.`

## Client Matrix

| Client | State | Scope | Allowed surface |
| --- | --- | --- | --- |
| `alex-web` | active local key | `dashboard` | five GET routes below |
| `zinga-broker` | future, not configured | `broker` | no route enabled |
| `codex` | MCP session identity | authorized MCP tools | no Linux/admin scope |
| legacy API | existing independent token | legacy API handlers | unchanged |
| admin | existing independent token | admin handlers | not granted to client keys |

No client receives a secret or direct database access. A new client or scope requires tests and documentation. No scope is implicitly inherited.

## ALEX Dashboard Scope

The only dashboard routes are:

- `GET /api/health`
- `GET /api/lab/openstack`
- `GET /api/lab/cabinets`
- `GET /api/lab/roes`
- `GET /api/lab/topology`

POST, PUT, PATCH and DELETE are not registered on this surface. Administration, internal routes, MCP writes and secret routes do not accept `dashboard`.

The local source is `FIBERSPIDER_ALEX_READ_TOKEN` in ignored `secrets/runtime.env` (mode 0600). Its metadata uses a separate `key_id`, `client_id=alex-web`, state and expiration. No token belongs in Git, tests, JavaScript, HTML, localStorage, URLs, logs or API responses.

## Key Lifecycle

States are `PENDING`, `ACTIVE`, `GRACE`, `REVOKED`, and `EXPIRED`. Only ACTIVE and unexpired GRACE keys authenticate. Unknown, revoked, expired and invalid keys are denied. Provider unavailability is 503/DENY; it never enables anonymous access or key generation.

The provider supports more than one key for a client. During rotation the old key enters GRACE while the new key is ACTIVE; clients migrate, health is verified, then the old key is revoked. Metadata carries client ID, key ID, scope, state and expiry. Audit records must use those identifiers and result, never the secret.

## Responses

- `401`: missing, malformed, unknown or invalid token.
- `403`: authenticated client without the required scope.
- `503`: credential provider unavailable; the request fails closed.

## Future Provider

`CredentialProvider` is the replacement boundary. `EnvCredentialProvider` is the current implementation. A future `ZingaCredentialProvider` can fetch versioned metadata and authorize issued credentials without changing routes or FTTH services. Metadata lookups are separate from secret material; application modules receive only a verified `ClientPrincipal`.
