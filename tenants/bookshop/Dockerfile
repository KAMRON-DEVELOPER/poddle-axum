FROM ghcr.io/astral-sh/uv:python3.14-alpine

ENV PYTHONUNBUFFERED=1

# OpenTelemetry Configuration
ENV OTEL_EXPORTER_OTLP_PROTOCOL='grpc'
ENV OTEL_EXPORTER_OTLP_ENDPOINT='https://alloy-gateway.poddle.uz:4317'
ENV OTEL_SERVICE_NAME='bookshop-service'

WORKDIR /app

# Copy dependency files first for caching
COPY ./pyproject.toml ./
COPY ./uv.lock ./

# Install dependencies
RUN uv lock
RUN uv sync --locked

# Copy application code
COPY main.py ./

EXPOSE 8000

# Run the app
CMD ["uv", "run", "uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]
