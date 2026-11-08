# FiberSpider Architecture

The current architecture and verified live status are maintained in
[ARCHITETTURA.md](ARCHITETTURA.md). It covers AAA/RAGNO/ALEX, the unverified ALE
role, Rust services, private Secure MCP Tunnel, the 112-tool inventory, physical
assets, legacy import, WR/photo catalogs, manual ONE acceptance and pending work.

Rust is the primary runtime. Python activation/demo code is legacy. Telegram uses
private ACTIVE technician identity, not operational Telegram OTP. The control-plane
principal is independent of technician identity.

MCP/API use authoritative AAA PostgreSQL over Yggdrasil. No local database.
Credentials, photos, tunnel profiles and encrypted backups remain machine-local
under ignored protected directories. No node branches were merged.

The lab models FTTH assets and validates/audits named operations, not physical
GPON provisioning or a fully completed end-to-end digital twin. See
[OBIETTIVI.md](OBIETTIVI.md) for completed, verified and pending items.