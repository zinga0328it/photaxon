# ChatGPT Secure MCP Tunnel

RAGNO's Lia MCP is a local stdio server. OpenAI Secure MCP Tunnel can launch it without a public listener or inbound firewall rule.

## Required OpenAI values

1. In OpenAI Platform > Tunnels, create or select a tunnel associated with the target ChatGPT workspace and note its `tunnel_id`.
2. In OpenAI Platform > Runtime API keys, create a runtime key with Tunnels Read + Use. Do not use an admin key.
3. Put both values in the ignored local file `secrets/ragno-chatgpt-mcp.env`:
   - `CONTROL_PLANE_API_KEY`: runtime key; keep it private.
   - `CONTROL_PLANE_TUNNEL_ID`: selected tunnel ID.
4. Install the supported `tunnel-client` binary from the OpenAI Platform Tunnels download. Place it at `~/.local/bin/tunnel-client` and verify its version.

The runtime key is not a ChatGPT login token and is not the RAGNO API/database secret in `secrets/runtime.env`.

## Run

Build the MCP binary, then start the client in a terminal that remains open:

```sh
cd /home/ragno/apps/photaxon
PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path rust/Cargo.toml --release --bin ragno-lia-mcp
set -a
. /home/ragno/apps/photaxon/secrets/ragno-chatgpt-mcp.env
set +a
"$HOME/.local/bin/tunnel-client" init \
  --sample sample_mcp_stdio_local \
  --profile fiberspider-ragno \
  --tunnel-id "$CONTROL_PLANE_TUNNEL_ID" \
  --mcp-command "/home/ragno/apps/photaxon/rust/target/release/ragno-lia-mcp --private-tunnel-admin"
"$HOME/.local/bin/tunnel-client" doctor --profile fiberspider-ragno --health.listen-addr 127.0.0.1:0
"$HOME/.local/bin/tunnel-client" run --profile fiberspider-ragno --health.listen-addr 127.0.0.1:0
```

Keep `CONTROL_PLANE_API_KEY` in the environment when running `doctor` and `run`; never pass it as a command argument or print it. Keep only one tunnel-client runtime active for this tunnel ID.

## Connect ChatGPT

Open <https://chatgpt.com/plugins>, choose **Add custom MCP server**, select **Tunnel**, and choose the same tunnel. The tunnel runtime key authenticates the tunnel client; do not enter it as a ChatGPT or RAGNO MCP bearer token. Keep the ChatGPT app private and grant access only to the intended account/workspace. Full MCP write actions require a ChatGPT plan/workspace that supports them; Pro currently supports read/fetch tools only.

No inbound port, public endpoint, or system service is configured by this guide. Starting `tunnel-client run` is the point at which the private MCP becomes reachable through the OpenAI tunnel.

The profile already exists: do not reinitialize/overwrite it during normal
restarts. Only the trusted parent tunnel-client run --profile fiberspider-ragno
enables the admin child. The current profile writes its loopback health URL to
a private machine-local file. Verify readiness/polling; one process per tunnel.

After release updates, open FiberSpider settings in ChatGPT and choose Aggiorna
strumenti. The real app shows 112 tools; inventory READ, legacy import and
splitter dry-run are verified from ChatGPT. No public/workspace publication was done.

Before updates preserve old binary/profile and an encrypted logical AAA backup
with SHA-256 manifest. Binary rollback may keep compatible added schema; never
drop new tables containing user data. Database restore is a distinct approved
operator action, not an automatic silent rollback. See ARCHITETTURA.md for
current capabilities, safety boundaries and pending work.
