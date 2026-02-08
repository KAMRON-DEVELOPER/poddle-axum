import uvicorn
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
from typing import List
import uuid
import os

PORT = int(os.getenv("PORT", "8000"))

app = FastAPI(title="Simple Full Stack Backend")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)

class Item(BaseModel):
    id: str
    title: str

class CreateItem(BaseModel):
    title: str

# In-memory storage
ITEMS: List[Item] = []

@app.get("/health")
def health():
    return {"status": "ok"}

@app.get("/items", response_model=List[Item])
def list_items():
    return ITEMS

@app.post("/items", response_model=Item)
def create_item(data: CreateItem):
    item = Item(id=str(uuid.uuid4()), title=data.title)
    ITEMS.append(item)
    return item

@app.delete("/items/{item_id}")
def delete_item(item_id: str):
    global ITEMS
    ITEMS = [i for i in ITEMS if i.id != item_id]
    return {"deleted": item_id}

if __name__ == "__main__":
    uvicorn.run(
        app,
        host="0.0.0.0",
        port=PORT,
        log_config=None,
    )
