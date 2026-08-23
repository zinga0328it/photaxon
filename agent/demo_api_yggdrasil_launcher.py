from __future__ import annotations

YGGDRASIL_HOST = "200:ddcf:41aa:9b1b:b321:9778:949f:e2e9"
YGGDRASIL_PORT = 8000


def run() -> None:
    import uvicorn

    uvicorn.run(
        "agent.demo_api:app",
        host=YGGDRASIL_HOST,
        port=YGGDRASIL_PORT,
        loop="auto",
        interface="asgi3",
        lifespan="on",
        access_log=False,
        log_level="info",
    )


if __name__ == "__main__":
    run()
