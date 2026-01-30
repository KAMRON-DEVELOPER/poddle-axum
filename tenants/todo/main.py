import json
import logging
import os
import sys
import time
from contextlib import asynccontextmanager

import uvicorn
from fastapi import FastAPI, Request
from logfmter import Logfmter
from loguru import logger as loguru_logger
from pydantic import BaseModel

# -----------------------------
# Configuration
# -----------------------------
PORT = int(os.getenv("PORT", "8000"))
SERVICE_NAME = os.getenv("SERVICE_NAME", "todo")

# -----------------------------
# Logging Setup
# -----------------------------

# Loguru Logger (JSON)
loguru_logger.remove()
loguru_logger.add(sys.stdout, serialize=True)

# Logfmt Logger
logfmt_logger = logging.getLogger("logfmt_app")
logfmt_logger.setLevel(logging.INFO)
logfmt_handler = logging.StreamHandler(sys.stdout)
logfmt_handler.setFormatter(
    Logfmter(
        keys=["level", "ts", "msg"],
        mapping={"level": "levelname", "ts": "asctime", "msg": "message"},
        datefmt="%Y-%m-%dT%H:%M:%S%z",
    )
)
logfmt_logger.addHandler(logfmt_handler)

# Text Logger
text_logger = logging.getLogger("legacy_app")
text_logger.setLevel(logging.INFO)
text_handler = logging.StreamHandler(sys.stdout)
text_handler.setFormatter(
    logging.Formatter("%(asctime)s - %(name)s - %(levelname)s - %(message)s")
)
text_logger.addHandler(text_handler)


# -----------------------------
# Data Models
# -----------------------------
class Todo(BaseModel):
    id: int
    title: str
    completed: str


# In-memory database
TODOS_DB = {
    1: {"id": 1, "title": "Finish Poddle PaaS", "completed": False},
    2: {"id": 2, "title": "Test Loki Logs", "completed": True},
    3: {"id": 3, "title": "Fix Rust Backend", "completed": False},
}


# -----------------------------
# FastAPI app
# -----------------------------
@asynccontextmanager
async def lifespan(_app: FastAPI):
    # Log startup in all formats to see what happens in Loki
    loguru_logger.info("✅ Service starting", service=SERVICE_NAME, event="startup")
    logfmt_logger.info(f"✅ Service {SERVICE_NAME} starting event=startup")
    text_logger.info(f"✅ Service {SERVICE_NAME} starting up (Legacy)")
    yield
    loguru_logger.info("✅ Service shutting down", event="shutdown")


app = FastAPI(title=f"{SERVICE_NAME.upper()} Service", lifespan=lifespan)

logging.info(
    f"🚀 Service {SERVICE_NAME.upper()} running on port {PORT}",
    extra={"service": SERVICE_NAME, "port": PORT},
)


# -----------------------------
# Routes
# -----------------------------
@app.get("/")
def root():
    loguru_logger.info("Root endpoint accessed", user_agent="browser")
    return {"status": "ok", "service": SERVICE_NAME}


@app.get("/log/text")
def generate_text_log():
    """Generates unstructured, messy text logs."""
    # 1. Plain print (very common in simple scripts)
    print(f"Direct Print: User accessed /log/text at {time.time()}")
    # 2. Standard logging
    text_logger.warning("Database connection query took 1.2s - optimization needed")
    return {"status": "sent_text"}


@app.get("/log/json")
def generate_json_log():
    """Generates clean, structured JSON logs."""
    loguru_logger.info(
        "Todo item created",
        user_id=101,
        action="create",
        item_id="uuid-555",
        duration_ms=120,
    )
    return {"status": "sent_json"}


@app.get("/log/logfmt")
def generate_logfmt_log():
    """Generates key=value logs."""
    logfmt_logger.info("payment processed amount=50.00 currency=USD user_id=42")
    return {"status": "sent_logfmt"}


@app.get("/log/mixed")
def generate_mixed_error():
    """Generates chaos: Text Error + Logfmt Info + JSON Exception."""
    text_logger.error("Connection pool exhausted (Retrying in 5s)")

    logfmt_logger.info("retry attempt=1 status=failed backoff=5s")

    try:
        1 / 0
    except ZeroDivisionError:
        loguru_logger.exception("Critical calculation error")

    return {"status": "sent_chaos"}


if __name__ == "__main__":
    uvicorn.run(
        app,
        host="0.0.0.0",
        port=PORT,
        log_config=None,
    )
