# Reproducible technical demo

Prerequisites: Git and Docker Engine with Docker Compose v2. No Telegram token,
OpenStack installation, GPON equipment or LLM API key is required.

```bash
git clone --branch rd-reproducible-demo https://github.com/zinga0328it/photaxon.git
cd photaxon
cp .env.example .env
docker compose up --build -d
curl http://127.0.0.1:8000/api/health
docker compose run --rm backend python -m demo.run_demo
```

Repeat after resetting the laboratory port:

```bash
docker compose exec postgres psql -U fiberspider_app -d fiberspider -c \
  "UPDATE demo_resources SET state='FREE', owner_transaction_id=NULL WHERE resource_id='PORT-A-01';"
docker compose run --rm backend python -m demo.run_demo
```

Clean reset: `docker compose down -v`.

The demo proves: active technician lookup, simulated cabinet event, advisory
proposal, deterministic validation, resource reservation, PostgreSQL persistence
and a diagnostic LED output derived from known state.

| Component | Demo status |
|---|---|
| Python workflow and deterministic validation | working software |
| PostgreSQL persistence and reservation | working software |
| Smart-cabinet telemetry input | simulated |
| LLM agent proposal | simulated advisory adapter |
| Telegram OTP | optional existing integration; excluded from base demo |
| OpenStack/Neutron | optional laboratory integration; excluded from base demo |
| Physical ONT, GPON/OLT, sensors and LEDs | future hardware |
| Physical provisioning | not implemented |

