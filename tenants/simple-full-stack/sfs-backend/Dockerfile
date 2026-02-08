FROM ghcr.io/astral-sh/uv:python3.14-alpine

ENV PYTHONUNBUFFERED=1

WORKDIR /app

# Copy dependency files first for caching
COPY ./pyproject.toml ./
COPY ./uv.lock ./

# Install dependencies
RUN uv lock
RUN uv sync --locked

# Copy application code
COPY main.py ./

ENV PORT=8000
EXPOSE ${PORT}

# Run the app
CMD ["sh", "-c", "uv run uvicorn main:app --host 0.0.0.0 --port ${PORT}"]
