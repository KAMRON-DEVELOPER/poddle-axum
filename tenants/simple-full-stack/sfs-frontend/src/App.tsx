import { useCallback, useEffect, useState, type ChangeEvent } from 'react';
import './App.css';

const API_URL: string = (import.meta.env.VITE_API_URL as string | undefined) || 'http://localhost:8000';

interface Item {
  id: string;
  title: string;
}

interface CreateItem {
  title: string;
}

async function fetchJson<T>(input: RequestInfo | URL, init?: RequestInit): Promise<T> {
  const res = await fetch(input, init);
  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(`Request failed: ${res.status} ${res.statusText}${text ? ` - ${text}` : ''}`);
  }
  return (await res.json()) as T;
}

async function fetchNoContent(input: RequestInfo | URL, init?: RequestInit): Promise<void> {
  const res = await fetch(input, init);
  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(`Request failed: ${res.status} ${res.statusText}${text ? ` - ${text}` : ''}`);
  }
}

function App() {
  const [items, setItems] = useState<Item[]>([]);
  const [title, setTitle] = useState('');

  const load = useCallback(async (): Promise<void> => {
    try {
      const data = await fetchJson<Item[]>(`${API_URL}/items`);
      setItems(data);
    } catch (err) {
      console.error(err);
      setItems([]);
    }
  }, []);

  const addItem = useCallback(async (): Promise<void> => {
    const nextTitle = title.trim();
    if (!nextTitle) return;

    const payload: CreateItem = { title: nextTitle };
    try {
      await fetchNoContent(`${API_URL}/items`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      setTitle('');
      await load();
    } catch (err) {
      console.error(err);
    }
  }, [load, title]);

  const removeItem = useCallback(
    async (id: Item['id']): Promise<void> => {
      try {
        await fetchNoContent(`${API_URL}/items/${id}`, { method: 'DELETE' });
        await load();
      } catch (err) {
        console.error(err);
      }
    },
    [load],
  );

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div
      className='app'
      style={{ padding: 40, fontFamily: 'sans-serif' }}>
      <h1>PaaS Test App</h1>

      <input
        className='item-input'
        value={title}
        onChange={(e: ChangeEvent<HTMLInputElement>) => setTitle(e.target.value)}
        placeholder='New item'
      />
      <button
        className='add-button'
        onClick={addItem}>
        Add
      </button>

      <ul className='item-list'>
        {items.map((i) => (
          <li
            className='item-row'
            key={i.id}>
            <span className='item-title'>{i.title}</span>
            <button
              className='delete-button'
              onClick={() => removeItem(i.id)}>
              x
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

export default App;
