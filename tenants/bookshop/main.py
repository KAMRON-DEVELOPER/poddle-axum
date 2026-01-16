from fastapi import HTTPException
from typing import Optional
from pydantic import BaseModel
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
# Configuration
# -----------------------------
PORT = int(os.getenv("PORT", "8000"))
SERVICE_NAME = os.getenv("OTEL_SERVICE_NAME", "bookshop-service")
ENDPOINT = os.getenv("OTEL_EXPORTER_OTLP_ENDPOINT", "https://alloy-gateway.poddle.uz:4317")

# -----------------------------
# Logging Setup
# -----------------------------
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] [%(otelTraceID)s] %(name)s - %(message)s",
)
logger = logging.getLogger(SERVICE_NAME)


# -----------------------------
# OpenTelemetry setup
# -----------------------------
logger.info(f"Initializing OpenTelemetry for {SERVICE_NAME} to {ENDPOINT}")

resource = Resource.create({
    "service.name": SERVICE_NAME,
    "service.version": "0.1.0",
    "deployment.environment": "development",
})

provider = TracerProvider(resource=resource)
# Using insecure=True/False depending on your gateway URL scheme (https usually implies secure, but often grpc uses insecure for internal proxies)
# If your endpoint is https, the exporter usually handles SSL. If raw grpc, check your gateway config.
exporter = OTLPSpanExporter(
    endpoint=ENDPOINT,
    insecure=True,
)
provider.add_span_processor(BatchSpanProcessor(exporter))
trace.set_tracer_provider(provider)
tracer = trace.get_tracer(__name__)


# -----------------------------
# Data Models
# -----------------------------
class Book(BaseModel):
    id: int
    title: str
    author: str
    price: float
    stock: int

class OrderRequest(BaseModel):
    book_id: int
    quantity: int
    customer_email: str

# In-memory database
BOOKS_DB  = {
    1: {"id": 1, "title": "The Great Gatsby", "author": "F. Scott Fitzgerald", "price": 12.99, "stock": 45},
    2: {"id": 2, "title": "1984", "author": "George Orwell", "price": 14.99, "stock": 32},
    3: {"id": 3, "title": "To Kill a Mockingbird", "author": "Harper Lee", "price": 13.50, "stock": 28},
    4: {"id": 4, "title": "Pride and Prejudice", "author": "Jane Austen", "price": 11.99, "stock": 52},
    5: {"id": 5, "title": "The Catcher in the Rye", "author": "J.D. Salinger", "price": 12.50, "stock": 19},
}


# -----------------------------
# FastAPI app
# -----------------------------
@asynccontextmanager
async def lifespan(_app: FastAPI):
    logger.info("📚 Bookshop service starting up")
    logger.info(f"Loaded {len(BOOKS_DB)} books into catalog") 
    yield
    logger.info("📚 Bookshop service shutting down")

app = FastAPI(title="Bookshop Service", lifespan=lifespan)
FastAPIInstrumentor.instrument_app(app)

logging.info(f"🚀 Service {SERVICE_NAME} running on port {PORT}")


# -----------------------------
# Helper Functions
# -----------------------------
def simulate_db_query(operation: str, delay_ms: Optional[int] = None):
    """Simulate database query with random delay"""
    if delay_ms is None:
        delay_ms = random.randint(10, 50)
    time.sleep(delay_ms / 1000)
    logger.debug(f"DB operation '{operation}' completed in {delay_ms}ms")

def simulate_cache_check(key: str) -> bool:
    """Simulate cache lookup"""
    hit = random.choice([True, False, False])  # 33% hit rate
    logger.debug(f"Cache lookup for '{key}': {'HIT' if hit else 'MISS'}")
    return hit


# -----------------------------
# Routes
# -----------------------------
@app.get("/")
def root():
    logger.info("Root endpoint called")
    with tracer.start_as_current_span("root_handler") as span:
        span.set_attribute("endpoint", "/")
        return {"status": "ok", "service": SERVICE_NAME, "version": "1.0.0"}

@app.get("/health")
def health():
    logger.info("Health check endpoint called")
    with tracer.start_as_current_span("health_check") as span:
        span.set_attribute("health_status", "healthy")
        return {"status": "healthy", "service": SERVICE_NAME}

@app.get("/books")
def list_books(author: Optional[str] = None, min_price: Optional[float] = None):
    logger.info(f"Listing books - filters: author={author}, min_price={min_price}")
    
    with tracer.start_as_current_span("list_books") as parent_span:
        parent_span.set_attribute("filter.author", author or "none")
        parent_span.set_attribute("filter.min_price", min_price or 0)
        
        # Simulate cache check
        with tracer.start_as_current_span("cache_lookup"):
            cache_hit = simulate_cache_check("books_list")
            if cache_hit:
                logger.info("Books list served from cache")
        
        # Simulate database query
        with tracer.start_as_current_span("database_query") as db_span:
            db_span.set_attribute("query_type", "SELECT")
            db_span.set_attribute("table", "books")
            simulate_db_query("SELECT * FROM books")
            
            books = list(BOOKS_DB.values())
            
            if author:
                books = [b for b in books if author.lower() in b["author"].lower()]
            if min_price:
                books = [b for b in books if b["price"] >= min_price]
            
            logger.info(f"Retrieved {len(books)} books from database")
            db_span.set_attribute("result_count", len(books))
        
        # Simulate response serialization
        with tracer.start_as_current_span("serialize_response"):
            time.sleep(0.005)  # 5ms serialization
            logger.debug("Response serialized successfully")
        
        parent_span.set_attribute("books_returned", len(books))
        logger.info(f"Returning {len(books)} books to client")
        
        return {"books": books, "count": len(books)}

@app.get("/books/{book_id}")
def get_book(book_id: int):
    logger.info(f"Fetching book with ID: {book_id}")
    
    with tracer.start_as_current_span("get_book") as parent_span:
        parent_span.set_attribute("book_id", book_id)
        
        # Simulate cache check
        with tracer.start_as_current_span("cache_lookup"):
            cache_hit = simulate_cache_check(f"book_{book_id}")
            if cache_hit:
                logger.info(f"Book {book_id} served from cache")
        
        # Simulate database query
        with tracer.start_as_current_span("database_query") as db_span:
            db_span.set_attribute("query_type", "SELECT")
            db_span.set_attribute("table", "books")
            db_span.set_attribute("book_id", book_id)
            simulate_db_query(f"SELECT * FROM books WHERE id={book_id}")
            
            if book_id not in BOOKS_DB:
                logger.warning(f"Book {book_id} not found")
                db_span.set_attribute("error", True)
                raise HTTPException(status_code=404, detail="Book not found")
            
            book = BOOKS_DB[book_id]
            logger.info(f"Retrieved book: {book['title']} by {book['author']}")
        
        return {"book": book}

@app.post("/orders")
def create_order(order: OrderRequest):
    logger.info(f"Processing order: book_id={order.book_id}, quantity={order.quantity}, customer={order.customer_email}")
    
    with tracer.start_as_current_span("create_order") as parent_span:
        parent_span.set_attribute("book_id", order.book_id)
        parent_span.set_attribute("quantity", order.quantity)
        parent_span.set_attribute("customer_email", order.customer_email)
        
        # Validate book exists
        with tracer.start_as_current_span("validate_book") as validate_span:
            simulate_db_query(f"SELECT * FROM books WHERE id={order.book_id}")
            
            if order.book_id not in BOOKS_DB:
                logger.error(f"Order failed: Book {order.book_id} not found")
                validate_span.set_attribute("error", True)
                raise HTTPException(status_code=404, detail="Book not found")
            
            book = BOOKS_DB[order.book_id]
            logger.info(f"Book validated: {book['title']}")
        
        # Check inventory
        with tracer.start_as_current_span("check_inventory") as inventory_span:
            simulate_db_query("SELECT stock FROM books")
            inventory_span.set_attribute("available_stock", book["stock"])
            inventory_span.set_attribute("requested_quantity", order.quantity)
            
            if book["stock"] < order.quantity:
                logger.warning(f"Insufficient stock: requested={order.quantity}, available={book['stock']}")
                inventory_span.set_attribute("error", True)
                raise HTTPException(status_code=400, detail="Insufficient stock")
            
            logger.info(f"Inventory check passed: {book['stock']} units available")
        
        # Calculate total
        with tracer.start_as_current_span("calculate_total") as calc_span:
            total = book["price"] * order.quantity
            calc_span.set_attribute("unit_price", book["price"])
            calc_span.set_attribute("total_amount", total)
            logger.info(f"Order total calculated: ${total:.2f}")
        
        # Process payment
        with tracer.start_as_current_span("process_payment") as payment_span:
            payment_span.set_attribute("amount", total)
            payment_span.set_attribute("payment_method", "credit_card")
            simulate_db_query("INSERT INTO payments", delay_ms=random.randint(100, 300))
            logger.info(f"Payment processed: ${total:.2f}")
        
        # Update inventory
        with tracer.start_as_current_span("update_inventory") as update_span:
            book["stock"] -= order.quantity
            simulate_db_query(f"UPDATE books SET stock={book['stock']}")
            update_span.set_attribute("new_stock", book["stock"])
            logger.info(f"Inventory updated: {book['stock']} units remaining")
        
        # Send confirmation email (simulated)
        with tracer.start_as_current_span("send_confirmation_email") as email_span:
            email_span.set_attribute("recipient", order.customer_email)
            time.sleep(0.05)  # Simulate email service call
            logger.info(f"Confirmation email sent to {order.customer_email}")
        
        order_id = random.randint(10000, 99999)
        parent_span.set_attribute("order_id", order_id)
        logger.info(f"Order {order_id} completed successfully")
        
        return {
            "order_id": order_id,
            "book": book["title"],
            "quantity": order.quantity,
            "total": total,
            "status": "confirmed"
        }

@app.get("/stats")
def get_stats():
    logger.info("Fetching bookshop statistics")
    
    with tracer.start_as_current_span("get_stats") as parent_span:
        # Calculate various statistics
        with tracer.start_as_current_span("calculate_inventory_stats"):
            simulate_db_query("SELECT SUM(stock) FROM books")
            total_stock = sum(book["stock"] for book in BOOKS_DB.values())
            logger.info(f"Total inventory: {total_stock} books")
        
        with tracer.start_as_current_span("calculate_value_stats"):
            simulate_db_query("SELECT AVG(price), MAX(price) FROM books")
            avg_price = sum(book["price"] for book in BOOKS_DB.values()) / len(BOOKS_DB)
            max_price = max(book["price"] for book in BOOKS_DB.values())
            logger.info(f"Average price: ${avg_price:.2f}, Max price: ${max_price:.2f}")
        
        with tracer.start_as_current_span("find_popular_books"):
            simulate_db_query("SELECT * FROM books ORDER BY sales DESC LIMIT 3")
            # Simulate popularity based on lower stock
            popular = sorted(BOOKS_DB.values(), key=lambda x: x["stock"])[:3]
            logger.info(f"Top selling books identified: {len(popular)} books")
        
        stats = {
            "total_books": len(BOOKS_DB),
            "total_stock": total_stock,
            "avg_price": round(avg_price, 2),
            "max_price": max_price,
            "popular_books": [b["title"] for b in popular]
        }
        
        parent_span.set_attribute("stats.total_books", stats["total_books"])
        logger.info("Statistics calculated successfully")
        
        return stats
    
if __name__ == "__main__":
    # Local debugging convenience
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=PORT)