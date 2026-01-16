# 📝 Notes Service

A note-taking microservice built with Express and TypeScript, featuring comprehensive logging and distributed tracing with OpenTelemetry.

## Service Name

**`notes-service`** - A RESTful API for creating and managing notes

## Features

- ✅ Full CRUD operations for notes
- ✅ Search and filtering by tags and content
- ✅ Archive/unarchive functionality
- ✅ Rich structured logging to stdout
- ✅ OpenTelemetry traces with nested spans
- ✅ In-memory database with seed data
- ✅ TypeScript for type safety

## Tech Stack

- **Express 5.x** - Web framework
- **TypeScript 5.x** - Type safety
- **OpenTelemetry** - Distributed tracing
- **Node.js 22** - Runtime

## API Endpoints

### `GET /`

Root health check

```json
{
  "status": "ok",
  "service": "notes-service",
  "version": "1.0.0",
  "notes_count": 4
}
```

### `GET /health`

Detailed health status

### `GET /notes`

List all notes with optional filters

- Query params:
  - `q` - Search in title/content
  - `tag` - Filter by tag (can be multiple)
  - `archived` - Show archived notes (true/false)

### `GET /notes/:id`

Get a specific note by ID

### `POST /notes`

Create a new note

```json
{
  "title": "My Note",
  "content": "Note content here",
  "tags": ["work", "important"]
}
```

### `PUT /notes/:id`

Update an existing note

```json
{
  "title": "Updated Title",
  "content": "Updated content",
  "tags": ["work"],
  "archived": false
}
```

### `DELETE /notes/:id`

Delete a note (returns 204 No Content)

### `GET /stats`

Get statistics about notes

- Total/active/archived counts
- Popular tags
- Average content length

## Observability

### Logs

Structured logs with different severity levels:

- **[INFO]** - Request handling, operations, results
- **[DEBUG]** - Cache hits/misses, database operations
- **[WARN]** - Not found errors
- **[ERROR]** - Validation failures, errors

### Traces

Each request creates detailed traces with multiple spans:

**List Notes** (`GET /notes`):

- `list_notes` (parent)
  - `cache_lookup`
  - `database_query`
  - `serialize_response`

**Create Note** (`POST /notes`):

- `create_note` (parent)
  - `validate_input`
  - `database_insert`
  - `cache_invalidate`
  - `update_search_index`

**Update Note** (`PUT /notes/:id`):

- `update_note` (parent)
  - `validate_input`
  - `fetch_existing`
  - `database_update`
  - `cache_invalidate`
  - `update_search_index`

**Delete Note** (`DELETE /notes/:id`):

- `delete_note` (parent)
  - `check_exists`
  - `database_delete`
  - `cache_invalidate`
  - `remove_from_search_index`

**Statistics** (`GET /stats`):

- `get_stats` (parent)
  - `count_notes`
  - `analyze_tags`
  - `analyze_content`

## Testing with cURL

```bash
# List all notes
curl http://localhost:3000/notes

# Search notes
curl "http://localhost:3000/notes?q=meeting"

# Filter by tag
curl "http://localhost:3000/notes?tag=work"

# Get specific note
curl http://localhost:3000/notes/1

# Create note
curl -X POST http://localhost:3000/notes \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Test Note",
    "content": "This is a test",
    "tags": ["test"]
  }'

# Update note
curl -X PUT http://localhost:3000/notes/1 \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Updated Title",
    "archived": true
  }'

# Delete note
curl -X DELETE http://localhost:3000/notes/1

# Get statistics
curl http://localhost:3000/stats
```

## Development

```bash
# Install dependencies
npm install

# Run in development mode with hot reload
npm run dev

# Build TypeScript
npm run build

# Run production build
npm start
```

## Docker

```bash
# Build image
docker build -t notes-service .

# Run container
docker run -p 3000:3000 notes-service

# With custom environment variables
docker run -p 3000:3000 \
  -e OTEL_SERVICE_NAME=my-notes \
  -e PORT=3000 \
  notes-service
```

## Environment Variables

- `PORT` - Server port (default: `3000`)
- `OTEL_EXPORTER_OTLP_ENDPOINT` - OpenTelemetry collector endpoint (`https://alloy-gateway.poddle.uz:4317`)
- `OTEL_SERVICE_NAME` - Service identifier for traces (`notes`)
- `NODE_ENV` - Environment (`development/staging/production`)

## Seed Data

The service starts with 4 sample notes:

1. Meeting Notes (work, meetings)
2. Shopping List (personal, todo)
3. Project Ideas (projects, ideas)
4. Book Recommendations (books, learning)

## Response Formats

All responses are in JSON format. Error responses follow this structure:

```json
{
  "error": "Error message here"
}
```

## Status Codes

- `200 OK` - Successful GET/PUT
- `201 Created` - Successful POST
- `204 No Content` - Successful DELETE
- `400 Bad Request` - Validation error
- `404 Not Found` - Resource not found
- `500 Internal Server Error` - Server error
