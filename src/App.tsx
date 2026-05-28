import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import "./App.css";

type Task = {
  id: string;
  title: string;
  done: boolean;
};

function App() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [draft, setDraft] = useState("");
  const [loadError, setLoadError] = useState<string | null>(null);

  const persist = useCallback(async (next: Task[]) => {
    await invoke("tasks_save", { tasks: next });
  }, []);

  useEffect(() => {
    (async () => {
      try {
        const loaded = await invoke<Task[]>("tasks_load");
        setTasks(loaded);
        setLoadError(null);
      } catch (e) {
        setLoadError(String(e));
      }
    })();
  }, []);

  const addTask = async () => {
    const title = draft.trim();
    if (!title) return;
    const next: Task[] = [
      ...tasks,
      { id: crypto.randomUUID(), title, done: false },
    ];
    setTasks(next);
    setDraft("");
    try {
      await persist(next);
      setLoadError(null);
    } catch (e) {
      setLoadError(String(e));
    }
  };

  const toggleTask = async (id: string) => {
    const next = tasks.map((t) =>
      t.id === id ? { ...t, done: !t.done } : t,
    );
    setTasks(next);
    try {
      await persist(next);
    } catch (e) {
      setLoadError(String(e));
    }
  };

  const removeTask = async (id: string) => {
    const next = tasks.filter((t) => t.id !== id);
    setTasks(next);
    try {
      await persist(next);
    } catch (e) {
      setLoadError(String(e));
    }
  };

  return (
    <div className="app">
      <header className="titlebar" data-tauri-drag-region>
        <span className="title">Задачи</span>
        <span className="hint">Ctrl+Shift+Space</span>
      </header>

      {loadError ? <p className="error">{loadError}</p> : null}

      <div className="composer">
        <input
          type="text"
          value={draft}
          placeholder="Новая задача…"
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void addTask();
          }}
        />
        <button type="button" onClick={() => void addTask()}>
          Добавить
        </button>
      </div>

      <ul className="task-list">
        {tasks.map((t) => (
          <li key={t.id} className={t.done ? "task done" : "task"}>
            <label>
              <input
                type="checkbox"
                checked={t.done}
                onChange={() => void toggleTask(t.id)}
              />
              <span className="task-title">{t.title}</span>
            </label>
            <button
              type="button"
              className="delete"
              onClick={() => void removeTask(t.id)}
              aria-label="Удалить"
            >
              ×
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

export default App;
