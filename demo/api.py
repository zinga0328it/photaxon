from fastapi import FastAPI, HTTPException
from demo.run_demo import run

app = FastAPI(title="FiberSpider Reproducible Demo API", version="1.0.0")

@app.get("/api/health")
def health() -> dict[str, str]:
    return {"status": "ok", "service": "fiberspider-reproducible-demo"}

@app.post("/api/demo/run")
def run_demo() -> dict[str, object]:
    try:
        return run()
    except RuntimeError as exc:
        raise HTTPException(status_code=409, detail=str(exc)) from exc

