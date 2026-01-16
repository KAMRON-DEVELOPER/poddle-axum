FROM ghcr.io/astral-sh/uv:python3.14-alpine

ENV PYTHONUNBUFFERED=1
ENV OTEL_EXPORTER_OTLP_PROTOCOL='grpc'
ENV OTEL_EXPORTER_OTLP_ENDPOINT='https://alloy-gateway.poddle.uz:4317'
ENV OTEL_SERVICE_NAME=''

WORKDIR /app

COPY ./pyproject.toml ./
COPY ./uv.lock ./
COPY main.py ./

EXPOSE 8000

RUN uv lock
RUN uv sync --locked

CMD ["uv", "run", "uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]
