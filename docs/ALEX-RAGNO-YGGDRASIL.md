# ALEX to RAGNO over Yggdrasil

## Trust Boundary

```text
Browser
  -> HTTPS
ALEX Apache (public edge, injects server-side token)
  -> Yggdrasil
RAGNO Rust API (client ID + scope authorization)
  -> OpenStack read-only APIs / AAA PostgreSQL
```

ALEX is a read-only dashboard client. Its token is not sent to JavaScript or the browser. RAGNO accepts it only for `client_id=alex-web`, `scope=dashboard` and the five GET endpoints listed below. No database or OpenStack secret is available to ALEX.

## RAGNO API

```text
RAGNO_API_PORT=18000
RAGNO_YGG_ADDRESS=[RAGNO_YGG_ADDRESS]
```

- `/api/health`
- `/api/lab/openstack`
- `/api/lab/cabinets`
- `/api/lab/roes`
- `/api/lab/topology`

RAGNO binds only to its Yggdrasil IPv6 address. The address above is a placeholder; get the currently authorized address through the node handoff.

## Token Storage and Transfer

RAGNO stores `FIBERSPIDER_ALEX_READ_TOKEN` only in ignored `secrets/runtime.env`, mode 0600. The value is not documented here. ALEX's destination is `/etc/photaxon/alex.env`, owner root, mode 0600, containing:

```dotenv
PHOTAXON_RAGNO_API_TOKEN=<SECRET_FROM_ENV>
```

Transfer the value through an authorized secure channel, entering it directly into a protected terminal prompt on ALEX. Never paste it into chat, shell arguments, Git, HTML, JavaScript, localStorage or a URL. Compare only the SHA-256 fingerprint delivered separately by RAGNO. If ALEX is unreachable, record `ALEX_SECRET_TRANSFER_REQUIRED=YES` and stop before public activation.

## Apache Requirements

Use the existing HTTPS `photaxon.com` vhost. Proxy the same-origin prefix `/fiberspider/api/` to `http://[RAGNO_YGG_ADDRESS]:18000/api/`. Apache must remove any client-supplied Authorization header and set a Bearer value from `PHOTAXON_RAGNO_API_TOKEN` server-side. Require `mod_proxy`, `mod_proxy_http`, and `mod_headers`; run `apache2ctl configtest` and a graceful reload. Do not change other vhosts, TLS, ports, firewall rules or Yggdrasil configuration.

The current ALEX vhost and public proxy are not verified/applied by this document. Verify all five public URLs and the dynamic homepage after ALEX applies the protected configuration.

Current transfer status: `ALEX_SECRET_TRANSFER_REQUIRED=YES`. No SSH/Yggdrasil
destination for ALEX was available in this session, so no secret was copied.
