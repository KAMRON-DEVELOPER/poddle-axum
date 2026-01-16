import { sdk, SERVICE_NAME } from './tracing.js';

import express, { type Application, type Request, type Response } from 'express';
import { trace, context, SpanStatusCode } from '@opentelemetry/api';

sdk.start();
console.log('[INFO] OpenTelemetry SDK initialized');

// -----------------------------
// Express App
// -----------------------------
const PORT = parseInt(process.env.PORT || '3000', 10);

const app: Application = express();
app.use(express.json());

// Logging middleware
app.use((req: Request, _res: Response, next) => {
  console.log(`[INFO] ${req.method} ${req.path} - Client: ${req.ip}`);
  next();
});

// ---------------------------
// Helpers
// ---------------------------
const logger = {
  debug: (msg: string) => console.log(`${new Date().toISOString()} [DEBUG] ${SERVICE_NAME} - ${msg}`),
  info: (msg: string) => console.log(`${new Date().toISOString()} [INFO] ${SERVICE_NAME} - ${msg}`),
  warn: (msg: string) => console.log(`${new Date().toISOString()} [WARN] ${SERVICE_NAME} - ${msg}`),
  error: (msg: string) => console.error(`${new Date().toISOString()} [ERROR] ${SERVICE_NAME} - ${msg}`),
};

const tracer = trace.getTracer(SERVICE_NAME, '1.0.0');

// -----------------------------
// Data Models
// -----------------------------
interface IdParams {
  id: string;
}

interface Note {
  id: number;
  title: string;
  content: string;
  tags: string[];
  created_at: string;
  updated_at: string;
  archived: boolean;
}

interface CreateNoteRequest {
  title: string;
  content: string;
  tags?: string[];
}

interface UpdateNoteRequest {
  title?: string;
  content?: string;
  tags?: string[];
  archived?: boolean;
}

// In-memory database
const notesDB = new Map<number, Note>();
let nextId = 1;

// Seed data
const seedNotes = [
  { title: 'Meeting Notes', content: 'Discussed Q1 roadmap and team priorities', tags: ['work', 'meetings'] },
  { title: 'Shopping List', content: 'Milk, eggs, bread, coffee beans', tags: ['personal', 'todo'] },
  { title: 'Project Ideas', content: 'Build a note-taking app with real-time sync', tags: ['projects', 'ideas'] },
  { title: 'Book Recommendations', content: 'The Pragmatic Programmer, Clean Code', tags: ['books', 'learning'] },
];

seedNotes.forEach((note) => {
  const now = new Date().toISOString();
  notesDB.set(nextId, {
    id: nextId,
    title: note.title,
    content: note.content,
    tags: note.tags,
    created_at: now,
    updated_at: now,
    archived: false,
  });
  nextId++;
});

// -----------------------------
// Helper Functions
// -----------------------------
function queryToString(value: unknown, fallback = 'none'): string {
  if (typeof value === 'string') return value;
  if (Array.isArray(value)) return value.join(',');
  return fallback;
}

function simulateDelay(operation: string, minMs = 5, maxMs = 30): void {
  const delay = Math.floor(Math.random() * (maxMs - minMs + 1)) + minMs;
  const start = Date.now();
  while (Date.now() - start < delay) {
    // Busy wait to simulate processing
  }
  logger.debug(`${operation} completed in ${delay}ms`);
}

function validateNote(note: CreateNoteRequest | UpdateNoteRequest): string | null {
  if ('title' in note && note.title !== undefined) {
    if (note.title.length === 0) return 'Title cannot be empty';
    if (note.title.length > 200) return 'Title too long (max 200 chars)';
  }
  if ('content' in note && note.content !== undefined) {
    if (note.content.length > 10000) return 'Content too long (max 10000 chars)';
  }
  if ('tags' in note && note.tags !== undefined) {
    if (note.tags.length > 10) return 'Too many tags (max 10)';
  }
  return null;
}

function searchNotes(query: string, tags?: string[]): Note[] {
  const results: Note[] = [];
  const lowerQuery = query.toLowerCase();

  for (const note of notesDB.values()) {
    if (note.archived) continue;

    // Check if query matches title or content
    const matchesQuery = !query || note.title.toLowerCase().includes(lowerQuery) || note.content.toLowerCase().includes(lowerQuery);

    // Check if all required tags are present
    const matchesTags = !tags || tags.length === 0 || tags.every((tag) => note.tags.includes(tag));

    if (matchesQuery && matchesTags) {
      results.push(note);
    }
  }

  return results;
}

// -----------------------------
// Routes
// -----------------------------

// Health check
app.get('/', (_req: Request, res: Response) => {
  tracer.startActiveSpan('root_handler', (span) => {
    try {
      span.setAttributes({
        'http.route': '/',
        'http.method': 'GET',
        'service.name': SERVICE_NAME,
        'notes.count': notesDB.size,
      });

      logger.info('Root endpoint called');

      res.json({
        status: 'ok',
        service: SERVICE_NAME,
        version: '1.0.0',
        notes_count: notesDB.size,
      });

      span.setAttribute('http.status_code', 200);
      span.setStatus({ code: SpanStatusCode.OK });
    } catch (err: any) {
      span.setStatus({
        code: SpanStatusCode.ERROR,
        message: err.message,
      });
      res.status(500).json({ error: 'Root handler failed' });
    } finally {
      span.end();
    }
  });
});

app.get('/health', (_req: Request, res: Response) => {
  tracer.startActiveSpan('health_check', (span) => {
    try {
      span.setAttributes({
        'http.route': '/health',
        'http.method': 'GET',
        'health.status': 'healthy',
        'notes.count': notesDB.size,
      });

      logger.info('Health check endpoint called');

      res.json({
        status: 'healthy',
        service: SERVICE_NAME,
      });

      span.setAttribute('http.status_code', 200);
      span.setStatus({ code: SpanStatusCode.OK });
    } catch (err: any) {
      span.setStatus({
        code: SpanStatusCode.ERROR,
        message: err.message,
      });
      res.status(500).json({ status: 'unhealthy' });
    } finally {
      span.end();
    }
  });
});

// List all notes
app.get('/notes', (req: Request, res: Response) => {
  tracer.startActiveSpan('list_notes', (parentSpan) => {
    try {
      const { q, tag, archived } = req.query;

      const tags = tag ? (Array.isArray(tag) ? (tag as string[]) : [tag as string]) : undefined;

      const showArchived = archived === 'true';

      const query = queryToString(q);
      const tagString = tags?.join(',') ?? 'none';

      parentSpan.setAttributes({
        'filter.query': query,
        'filter.tags': tagString,
        'filter.archived': showArchived,
      });

      const cacheSpan = tracer.startSpan('cache_lookup');
      simulateDelay('cache_check', 2, 8);
      cacheSpan.end();

      const dbSpan = tracer.startSpan('database_query');
      simulateDelay('database_query', 10, 40);

      let notes = Array.from(notesDB.values());

      if (!showArchived) {
        notes = notes.filter((n) => !n.archived);
      }

      if (q || tags) {
        notes = searchNotes((q as string) || '', tags);
      }

      dbSpan.setAttribute('result_count', notes.length);
      dbSpan.end();

      const serializeSpan = tracer.startSpan('serialize_response');
      simulateDelay('serialization', 3, 10);
      serializeSpan.end();

      parentSpan.setAttribute('notes_returned', notes.length);
      parentSpan.setStatus({ code: SpanStatusCode.OK });

      res.json({ notes, count: notes.length });
    } catch (err: any) {
      parentSpan.setStatus({
        code: SpanStatusCode.ERROR,
        message: err.message,
      });
      res.status(500).json({ error: 'Failed to list notes' });
    } finally {
      parentSpan.end();
    }
  });
});

// Get single note
app.get('/notes/:id', (req: Request<IdParams>, res: Response) => {
  tracer.startActiveSpan('get_note', (parentSpan) => {
    try {
      const id = parseInt(req.params.id, 10);
      parentSpan.setAttribute('note_id', id);

      const dbSpan = tracer.startSpan('database_query');
      simulateDelay('database_query', 10, 30);

      const note = notesDB.get(id);

      if (!note) {
        parentSpan.setStatus({
          code: SpanStatusCode.ERROR,
          message: 'Note not found',
        });
        res.status(404).json({ error: 'Note not found' });
        return;
      }

      dbSpan.end();
      parentSpan.setStatus({ code: SpanStatusCode.OK });
      res.json({ note });
    } catch (err: any) {
      parentSpan.setStatus({
        code: SpanStatusCode.ERROR,
        message: err.message,
      });
      res.status(500).json({ error: 'Failed to fetch note' });
    } finally {
      parentSpan.end();
    }
  });
});

// Create new note
app.post('/notes', (req: Request, res: Response) => {
  tracer.startActiveSpan('create_note', (parentSpan) => {
    try {
      const { title, content, tags = [] } = req.body as CreateNoteRequest;

      parentSpan.setAttributes({
        'note.title': title,
        'note.tags_count': tags.length,
      });

      const validationError = validateNote({ title, content, tags });
      if (validationError) {
        parentSpan.setStatus({
          code: SpanStatusCode.ERROR,
          message: validationError,
        });
        res.status(400).json({ error: validationError });
        return;
      }

      const now = new Date().toISOString();
      const note: Note = {
        id: nextId++,
        title,
        content,
        tags,
        created_at: now,
        updated_at: now,
        archived: false,
      };

      notesDB.set(note.id, note);

      parentSpan.setAttribute('note_id', note.id);
      parentSpan.setStatus({ code: SpanStatusCode.OK });

      res.status(201).json({ note });
    } catch (err: any) {
      parentSpan.setStatus({
        code: SpanStatusCode.ERROR,
        message: err.message,
      });
      res.status(500).json({ error: 'Failed to create note' });
    } finally {
      parentSpan.end();
    }
  });
});

// Update note
app.put('/notes/:id', (req: Request<IdParams>, res: Response) => {
  tracer.startActiveSpan('update_note', (parentSpan) => {
    try {
      const id = parseInt(req.params.id, 10);
      const updates = req.body as UpdateNoteRequest;

      parentSpan.setAttribute('note_id', id);

      const validationError = validateNote(updates);
      if (validationError) {
        parentSpan.setStatus({
          code: SpanStatusCode.ERROR,
          message: validationError,
        });
        res.status(400).json({ error: validationError });
        return;
      }

      const note = notesDB.get(id);
      if (!note) {
        parentSpan.setStatus({
          code: SpanStatusCode.ERROR,
          message: 'Note not found',
        });
        res.status(404).json({ error: 'Note not found' });
        return;
      }

      const updatedNote = {
        ...note,
        ...updates,
        updated_at: new Date().toISOString(),
      };

      notesDB.set(id, updatedNote);

      parentSpan.setStatus({ code: SpanStatusCode.OK });
      res.json({ note: updatedNote });
    } catch (err: any) {
      parentSpan.setStatus({
        code: SpanStatusCode.ERROR,
        message: err.message,
      });
      res.status(500).json({ error: 'Failed to update note' });
    } finally {
      parentSpan.end();
    }
  });
});

// Delete note
app.delete('/notes/:id', (req: Request<IdParams>, res: Response) => {
  tracer.startActiveSpan('delete_note', (parentSpan) => {
    try {
      const id = parseInt(req.params.id, 10);
      parentSpan.setAttribute('note_id', id);

      const checkSpan = tracer.startSpan('check_exists');
      simulateDelay('database_query', 10, 25);

      const note = notesDB.get(id);

      if (!note) {
        checkSpan.setStatus({
          code: SpanStatusCode.ERROR,
          message: 'Note not found',
        });
        checkSpan.end();

        parentSpan.setStatus({
          code: SpanStatusCode.ERROR,
          message: 'Note not found',
        });

        res.status(404).json({ error: 'Note not found' });
        return;
      }

      checkSpan.end();

      const deleteSpan = tracer.startSpan('database_delete');
      deleteSpan.setAttribute('query_type', 'DELETE');
      simulateDelay('database_delete', 15, 40);

      notesDB.delete(id);
      deleteSpan.end();

      const cacheSpan = tracer.startSpan('cache_invalidate');
      simulateDelay('cache_invalidate', 3, 10);
      cacheSpan.end();

      const indexSpan = tracer.startSpan('remove_from_search_index');
      simulateDelay('indexing', 5, 15);
      indexSpan.end();

      parentSpan.setStatus({ code: SpanStatusCode.OK });
      res.status(204).send();
    } catch (err: any) {
      parentSpan.setStatus({
        code: SpanStatusCode.ERROR,
        message: err.message,
      });
      res.status(500).json({ error: 'Failed to delete note' });
    } finally {
      parentSpan.end();
    }
  });
});

// Get statistics
app.get('/stats', (_req: Request, res: Response) => {
  tracer.startActiveSpan('get_stats', (parentSpan) => {
    try {
      const notes = Array.from(notesDB.values());

      const totalNotes = notes.length;
      const archivedNotes = notes.filter((n) => n.archived).length;

      parentSpan.setAttribute('total_notes', totalNotes);
      parentSpan.setStatus({ code: SpanStatusCode.OK });

      res.json({
        total_notes: totalNotes,
        archived_notes: archivedNotes,
        active_notes: totalNotes - archivedNotes,
      });
    } catch (err: any) {
      parentSpan.setStatus({
        code: SpanStatusCode.ERROR,
        message: err.message,
      });
      res.status(500).json({ error: 'Failed to fetch stats' });
    } finally {
      parentSpan.end();
    }
  });
});

// -----------------------------
// Start Server
// -----------------------------
const server = app.listen(PORT, () => {
  logger.info(`🚀 ${SERVICE_NAME} listening on port ${PORT}`);
  logger.info(`Seeded with ${notesDB.size} sample notes`);
});

// Graceful shutdown
process.on('SIGTERM', () => {
  console.log('[INFO] SIGTERM received, shutting down gracefully');
  server.close(() => {
    console.log('[INFO] Server closed');
    sdk.shutdown().then(
      () => {
        console.log('[INFO] OpenTelemetry SDK shut down successfully');
        process.exit(0);
      },
      (err) => console.log('[ERROR] Error shutting down OpenTelemetry SDK', err)
    );
  });
});
