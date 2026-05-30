import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import "./App.css";

type Task = {
  id: number;
  title: string;
  done: boolean;
  completed_at: string | null;
};

function sortTasks(tasks: Task[]): Task[] {
  return [...tasks].sort((a, b) => {
    if (a.done !== b.done) return a.done ? 1 : -1;
    if (!a.done) return b.id - a.id;
    return (b.completed_at ?? "").localeCompare(a.completed_at ?? "");
  });
}

function App() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [draft, setDraft] = useState("");
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const loaded = await invoke<Task[]>("tasks_load");
        setTasks(sortTasks(loaded));
        setLoadError(null);
      } catch (e) {
        setLoadError(String(e));
      }
    })();
  }, []);

  const addTask = async () => {
    const title = draft.trim();
    if (!title) return;
    try {
      const created = await invoke<Task>("task_create", { title });
      setTasks((prev) => sortTasks([...prev, created]));
      setDraft("");
      setLoadError(null);
    } catch (e) {
      setLoadError(String(e));
    }
  };

  const toggleTask = async (id: number) => {
    const task = tasks.find((t) => t.id === id);
    if (!task) return;
    const done = !task.done;
    const prev = tasks;
    setTasks(
      sortTasks(
        tasks.map((item) =>
          item.id === id
            ? {
                ...item,
                done,
                completed_at: done ? new Date().toISOString() : null,
              }
            : item,
        ),
      ),
    );
    try {
      const updated = await invoke<Task>("task_set_done", { id, done });
      setTasks((current) =>
        sortTasks(current.map((item) => (item.id === id ? updated : item))),
      );
      setLoadError(null);
    } catch (e) {
      setTasks(prev);
      setLoadError(String(e));
    }
  };

  const removeTask = async (id: number) => {
    const prev = tasks;
    setTasks((t) => t.filter((item) => item.id !== id));
    try {
      await invoke("task_delete", { id });
      setLoadError(null);
    } catch (e) {
      setTasks(prev);
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
