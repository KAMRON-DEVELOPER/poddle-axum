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
  const span = tracer.startSpan('root_handler');
  logger.info('Root endpoint called');

  span.setAttribute('endpoint', '/');
  span.setAttribute('service', SERVICE_NAME);

  res.json({
    status: 'ok',
    service: SERVICE_NAME,
    version: '1.0.0',
    notes_count: notesDB.size,
  });

  span.end();
});

app.get('/health', (_req: Request, res: Response) => {
  const span = tracer.startSpan('health_check');
  logger.info('Health check endpoint called');

  span.setAttribute('health_status', 'healthy');
  span.setAttribute('notes_count', notesDB.size);

  res.json({ status: 'healthy', service: SERVICE_NAME });

  span.end();
});

// List all notes
app.get('/notes', (req: Request, res: Response) => {
  const { q, tag, archived } = req.query;
  const tags = tag ? (Array.isArray(tag) ? (tag as string[]) : [tag as string]) : undefined;
  const showArchived = archived === 'true';

  logger.info(`Listing notes - query: ${q || 'none'}, tags: ${tags?.join(',') || 'none'}, archived: ${showArchived}`);

  const parentSpan = tracer.startSpan('list_notes');
  parentSpan.setAttribute('filter.query', (q as string) || 'none');
  parentSpan.setAttribute('filter.tags', tags?.join(',') || 'none');
  parentSpan.setAttribute('filter.archived', showArchived);

  // Simulate cache check
  const cacheSpan = tracer.startSpan('cache_lookup', {}, context.active());
  const cacheHit = Math.random() < 0.4;
  cacheSpan.setAttribute('cache_hit', cacheHit);
  logger.debug(`Cache lookup: ${cacheHit ? 'HIT' : 'MISS'}`);
  simulateDelay('cache_check', 2, 8);
  cacheSpan.end();

  // Simulate database query
  const dbSpan = tracer.startSpan('database_query', {}, context.active());
  dbSpan.setAttribute('query_type', 'SELECT');
  dbSpan.setAttribute('table', 'notes');
  simulateDelay('database_query', 10, 40);

  let notes = Array.from(notesDB.values());

  if (!showArchived) {
    notes = notes.filter((n) => !n.archived);
  }

  if (q || tags) {
    notes = searchNotes((q as string) || '', tags);
  }

  logger.info(`Retrieved ${notes.length} notes from database`);
  dbSpan.setAttribute('result_count', notes.length);
  dbSpan.end();

  // Simulate response serialization
  const serializeSpan = tracer.startSpan('serialize_response', {}, context.active());
  simulateDelay('serialization', 3, 10);
  serializeSpan.end();

  parentSpan.setAttribute('notes_returned', notes.length);
  parentSpan.end();

  logger.info(`Returning ${notes.length} notes to client`);
  res.json({ notes, count: notes.length });
});

// Get single note
app.get('/notes/:id', (req: Request<IdParams>, res: Response) => {
  const id = parseInt(req.params.id, 10);
  logger.info(`Fetching note with ID: ${id}`);

  const parentSpan = tracer.startSpan('get_note');
  parentSpan.setAttribute('note_id', id);

  // Simulate cache check
  const cacheSpan = tracer.startSpan('cache_lookup', {}, context.active());
  const cacheHit = Math.random() < 0.5;
  cacheSpan.setAttribute('cache_hit', cacheHit);
  logger.debug(`Cache lookup for note ${id}: ${cacheHit ? 'HIT' : 'MISS'}`);
  simulateDelay('cache_check', 2, 8);
  cacheSpan.end();

  // Simulate database query
  const dbSpan = tracer.startSpan('database_query', {}, context.active());
  dbSpan.setAttribute('query_type', 'SELECT');
  dbSpan.setAttribute('note_id', id);
  simulateDelay('database_query', 10, 30);

  const note = notesDB.get(id);

  if (!note) {
    logger.warn(`Note ${id} not found`);
    dbSpan.setAttribute('error', true);
    dbSpan.setStatus({ code: SpanStatusCode.ERROR, message: 'Note not found' });
    dbSpan.end();
    parentSpan.end();
    res.status(404).json({ error: 'Note not found' });
    return;
  }

  logger.info(`Retrieved note: "${note.title}"`);
  dbSpan.end();
  parentSpan.end();

  res.json({ note });
});

// Create new note
app.post('/notes', (req: Request, res: Response) => {
  const { title, content, tags = [] } = req.body as CreateNoteRequest;

  logger.info(`Creating new note: "${title}"`);

  const parentSpan = tracer.startSpan('create_note');
  parentSpan.setAttribute('note_title', title);
  parentSpan.setAttribute('tags_count', tags.length);

  // Validate input
  const validateSpan = tracer.startSpan('validate_input', {}, context.active());
  const validationError = validateNote({ title, content, tags });
  simulateDelay('validation', 2, 5);

  if (validationError) {
    logger.error(`Validation failed: ${validationError}`);
    validateSpan.setAttribute('error', true);
    validateSpan.setStatus({ code: SpanStatusCode.ERROR, message: validationError });
    validateSpan.end();
    parentSpan.end();
    res.status(400).json({ error: validationError });
    return;
  }

  logger.debug('Validation passed');
  validateSpan.end();

  // Create note
  const dbSpan = tracer.startSpan('database_insert', {}, context.active());
  dbSpan.setAttribute('query_type', 'INSERT');
  simulateDelay('database_insert', 15, 50);

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
  logger.info(`Note created with ID: ${note.id}`);
  dbSpan.setAttribute('note_id', note.id);
  dbSpan.end();

  // Invalidate cache
  const cacheSpan = tracer.startSpan('cache_invalidate', {}, context.active());
  simulateDelay('cache_invalidate', 3, 10);
  logger.debug('Cache invalidated');
  cacheSpan.end();

  // Index for search
  const indexSpan = tracer.startSpan('update_search_index', {}, context.active());
  simulateDelay('indexing', 5, 15);
  logger.debug('Search index updated');
  indexSpan.end();

  parentSpan.setAttribute('note_id', note.id);
  parentSpan.end();

  logger.info(`Note ${note.id} created successfully`);
  res.status(201).json({ note });
});

// Update note
app.put('/notes/:id', (req: Request<IdParams>, res: Response) => {
  const id = parseInt(req.params.id, 10);
  const updates = req.body as UpdateNoteRequest;

  logger.info(`Updating note ${id}`);

  const parentSpan = tracer.startSpan('update_note');
  parentSpan.setAttribute('note_id', id);

  // Validate input
  const validateSpan = tracer.startSpan('validate_input', {}, context.active());
  const validationError = validateNote(updates);
  simulateDelay('validation', 2, 5);

  if (validationError) {
    logger.error(`Validation failed: ${validationError}`);
    validateSpan.setAttribute('error', true);
    validateSpan.setStatus({ code: SpanStatusCode.ERROR, message: validationError });
    validateSpan.end();
    parentSpan.end();
    res.status(400).json({ error: validationError });
    return;
  }

  validateSpan.end();

  // Fetch existing note
  const fetchSpan = tracer.startSpan('fetch_existing', {}, context.active());
  simulateDelay('database_query', 10, 30);

  const note = notesDB.get(id);

  if (!note) {
    logger.warn(`Note ${id} not found`);
    fetchSpan.setAttribute('error', true);
    fetchSpan.setStatus({ code: SpanStatusCode.ERROR, message: 'Note not found' });
    fetchSpan.end();
    parentSpan.end();
    res.status(404).json({ error: 'Note not found' });
    return;
  }

  fetchSpan.end();

  // Update note
  const updateSpan = tracer.startSpan('database_update', {}, context.active());
  updateSpan.setAttribute('query_type', 'UPDATE');
  simulateDelay('database_update', 15, 45);

  const updatedNote: Note = {
    ...note,
    ...updates,
    updated_at: new Date().toISOString(),
  };

  notesDB.set(id, updatedNote);
  logger.info(`Note ${id} updated successfully`);
  updateSpan.end();

  // Invalidate cache
  const cacheSpan = tracer.startSpan('cache_invalidate', {}, context.active());
  simulateDelay('cache_invalidate', 3, 10);
  logger.debug('Cache invalidated');
  cacheSpan.end();

  // Update search index
  const indexSpan = tracer.startSpan('update_search_index', {}, context.active());
  simulateDelay('indexing', 5, 15);
  logger.debug('Search index updated');
  indexSpan.end();

  parentSpan.end();
  res.json({ note: updatedNote });
});

// Delete note
app.delete('/notes/:id', (req: Request<IdParams>, res: Response) => {
  const id = parseInt(req.params.id, 10);
  logger.info(`Deleting note ${id}`);

  const parentSpan = tracer.startSpan('delete_note');
  parentSpan.setAttribute('note_id', id);

  // Check if note exists
  const checkSpan = tracer.startSpan('check_exists', {}, context.active());
  simulateDelay('database_query', 10, 25);

  const note = notesDB.get(id);

  if (!note) {
    logger.warn(`Note ${id} not found`);
    checkSpan.setAttribute('error', true);
    checkSpan.setStatus({ code: SpanStatusCode.ERROR, message: 'Note not found' });
    checkSpan.end();
    parentSpan.end();
    res.status(404).json({ error: 'Note not found' });
    return;
  }

  checkSpan.end();

  // Delete from database
  const deleteSpan = tracer.startSpan('database_delete', {}, context.active());
  deleteSpan.setAttribute('query_type', 'DELETE');
  simulateDelay('database_delete', 15, 40);

  notesDB.delete(id);
  logger.info(`Note ${id} deleted from database`);
  deleteSpan.end();

  // Invalidate cache
  const cacheSpan = tracer.startSpan('cache_invalidate', {}, context.active());
  simulateDelay('cache_invalidate', 3, 10);
  logger.debug('Cache invalidated');
  cacheSpan.end();

  // Remove from search index
  const indexSpan = tracer.startSpan('remove_from_search_index', {}, context.active());
  simulateDelay('indexing', 5, 15);
  logger.debug('Removed from search index');
  indexSpan.end();

  parentSpan.end();
  logger.info(`Note ${id} deleted successfully`);
  res.status(204).send();
});

// Get statistics
app.get('/stats', (_req: Request, res: Response) => {
  logger.info('Fetching notes statistics');

  const parentSpan = tracer.startSpan('get_stats');

  // Count notes
  const countSpan = tracer.startSpan('count_notes', {}, context.active());
  simulateDelay('database_query', 10, 30);

  const notes = Array.from(notesDB.values());
  const totalNotes = notes.length;
  const archivedNotes = notes.filter((n) => n.archived).length;
  const activeNotes = totalNotes - archivedNotes;

  logger.info(`Total notes: ${totalNotes}, Active: ${activeNotes}, Archived: ${archivedNotes}`);
  countSpan.end();

  // Analyze tags
  const tagsSpan = tracer.startSpan('analyze_tags', {}, context.active());
  simulateDelay('data_processing', 15, 40);

  const tagCounts = new Map<string, number>();
  notes.forEach((note) => {
    note.tags.forEach((tag) => {
      tagCounts.set(tag, (tagCounts.get(tag) || 0) + 1);
    });
  });

  const popularTags = Array.from(tagCounts.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5)
    .map(([tag, count]) => ({ tag, count }));

  logger.info(`Found ${tagCounts.size} unique tags`);
  tagsSpan.end();

  // Calculate average content length
  const analyzeSpan = tracer.startSpan('analyze_content', {}, context.active());
  simulateDelay('data_processing', 10, 25);

  const avgContentLength = Math.round(notes.reduce((sum, note) => sum + note.content.length, 0) / (totalNotes || 1));

  logger.info(`Average content length: ${avgContentLength} characters`);
  analyzeSpan.end();

  const stats = {
    total_notes: totalNotes,
    active_notes: activeNotes,
    archived_notes: archivedNotes,
    total_tags: tagCounts.size,
    popular_tags: popularTags,
    avg_content_length: avgContentLength,
  };

  parentSpan.setAttribute('total_notes', totalNotes);
  parentSpan.end();

  logger.info('Statistics calculated successfully');
  res.json(stats);
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
