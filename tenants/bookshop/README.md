# 📚 Bookshop Service

A demo microservice for the Poddle PaaS platform with comprehensive logging and distributed tracing.

## Service Name

**`bookshop-service`** - A book retail API with inventory management

## Features

- ✅ Rich structured logging to stdout
- ✅ OpenTelemetry traces with multiple nested spans
- ✅ RESTful API with realistic business logic
- ✅ In-memory database for testing
- ✅ Simulated external dependencies (cache, database, payment, email)

## API Endpoints

### `GET /`

Health check - returns service status

### `GET /health`

Detailed health information

### `GET /books`

List all books with optional filters

- Query params: `author`, `min_price`
- Returns: Array of books with inventory info

### `GET /books/{book_id}`

Get specific book details

- Returns: Single book object

### `POST /orders`

Create a new order

```json
{
  "book_id": 1,
  "quantity": 2,
  "customer_email": "customer@example.com"
}
```

- Validates inventory
- Processes payment
- Updates stock
- Sends confirmation email

### `GET /stats`

Get bookshop statistics

- Total inventory
- Pricing analytics
- Popular books

## Observability

### Logs

All operations emit structured logs showing:

- Request parameters
- Database queries
- Cache hits/misses
- Business logic decisions
- Error conditions

### Traces

Each request creates a trace with multiple spans:

- **Root span**: Overall request handling
- **Nested spans**: Database queries, cache lookups, external service calls
- **Attributes**: Request metadata, query parameters, business metrics

## Testing with cURL

```bash
# List books
curl http://localhost:8000/books

# Get specific book
curl http://localhost:8000/books/1

# Create order
curl -X POST http://localhost:8000/orders \
  -H "Content-Type: application/json" \
  -d '{"book_id": 1, "quantity": 2, "customer_email": "test@example.com"}'

# Get statistics
curl http://localhost:8000/stats
```

## Environment Variables

- `PORT`: Server port (default: `8000`)
- `OTEL_EXPORTER_OTLP_ENDPOINT`: OpenTelemetry collector endpoint (`https://alloy-gateway.poddle.uz:4317`)
- `OTEL_SERVICE_NAME`: Service identifier for traces (`bookshop-service`)
- `OTEL_EXPORTER_OTLP_PROTOCOL`: Protocol (`grpc`)

## Deployment

```bash
docker build -t bookshop-service .
docker run -p 8000:8000 bookshop-service
```
