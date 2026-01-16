from contextlib import asynccontextmanager
import os
import time
import random
import logging

from fastapi import FastAPI
from opentelemetry import trace
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
from opentelemetry.instrumentation.fastapi import FastAPIInstrumentor

# -----------------------------
# Logging (stdout only)
# -----------------------------
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s level=%(levelname)s message=%(message)s",
)
logger = logging.getLogger("e-commerce")

# -----------------------------
# OpenTelemetry setup
# -----------------------------
port = int(os.getenv("PORT", "8000"))
endpoint = os.getenv(
    "OTEL_EXPORTER_OTLP_ENDPOINT",
    "https://alloy-gateway.poddle.uz:4317",
)
service_name = os.getenv("OTEL_SERVICE_NAME", "poddle-demo-app")

logger.info(f"Initializing OpenTelemetry with endpoint: {endpoint}")
logger.info(f"Service name: {service_name}")

resource = Resource.create({
    "service.name": service_name,
    "service.version": "0.1.0",
})

provider = TracerProvider(resource=resource)
exporter = OTLPSpanExporter(
    endpoint=endpoint,
    insecure=False,
)
provider.add_span_processor(BatchSpanProcessor(exporter))
trace.set_tracer_provider(provider)
tracer = trace.get_tracer(__name__)

# -----------------------------
# FastAPI app
# -----------------------------
@asynccontextmanager
async def lifespan(_app: FastAPI):
    logger.info("app_lifespan start") 
    yield
    logger.info("app_lifespan shutdown")

logging.info(f"🚀 Service {service_name} running on port {port}")

app = FastAPI(lifespan=lifespan)
FastAPIInstrumentor.instrument_app(app)


@app.get("/")
def root():
    logger.info("root endpoint called")
    return {"status": "ok"}


@app.get("/work")
def do_work():
    logger.info("work started")

    with tracer.start_as_current_span("work.parent") as parent:
        parent.set_attribute("work.type", "demo")
        parent.set_attribute("tenant.id", "example-tenant")

        for i in range(3):
            with tracer.start_as_current_span(f"work.child.{i}") as child:
                delay = random.uniform(0.1, 0.5)
                child.set_attribute("iteration", i)
                child.set_attribute("delay.seconds", delay)
                time.sleep(delay)

                logger.info(
                    "child step completed",
                    extra={"iteration": i, "delay": delay},
                )

        # simulate error span
        if random.random() < 0.3:
            try:
                raise RuntimeError("simulated failure")
            except Exception as e:
                parent.record_exception(e)
                parent.set_status(trace.Status(trace.StatusCode.ERROR))
                logger.error("simulated error occurred")

    logger.info("work finished")
    return {"result": "done"}
