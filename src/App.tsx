import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  type DragEndEvent,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import "./App.css";

type Task = {
  id: number;
  title: string;
  done: boolean;
  completed_at: string | null;
  position: number;
};

type TaskMoveResult = {
  task: Task;
  rebalanced: boolean;
};

function orderTasks(tasks: Task[]): Task[] {
  const active = tasks
    .filter((t) => !t.done)
    .sort((a, b) => b.position - a.position);
  const done = tasks
    .filter((t) => t.done)
    .sort((a, b) =>
      (b.completed_at ?? "").localeCompare(a.completed_at ?? ""),
    );
  return [...active, ...done];
}

function SortableActiveRow({
  task,
  onToggle,
  onDelete,
}: {
  task: Task;
  onToggle: (id: number) => void;
  onDelete: (id: number) => void;
}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: task.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <li
      ref={setNodeRef}
      style={style}
      className={isDragging ? "task task-dragging" : "task"}
      {...attributes}
      {...listeners}
    >
      <input
        type="checkbox"
        checked={task.done}
        onPointerDown={(e) => e.stopPropagation()}
        onClick={(e) => e.stopPropagation()}
        onChange={() => onToggle(task.id)}
      />
      <span className="task-title">{task.title}</span>
      <button
        type="button"
        className="delete"
        onPointerDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          onDelete(task.id);
        }}
        aria-label="Удалить"
      >
        ×
      </button>
    </li>
  );
}

function DoneRow({
  task,
  onToggle,
  onDelete,
}: {
  task: Task;
  onToggle: (id: number) => void;
  onDelete: (id: number) => void;
}) {
  return (
    <li className="task done">
      <input
        type="checkbox"
        checked={task.done}
        onChange={() => onToggle(task.id)}
      />
      <span className="task-title">{task.title}</span>
      <button
        type="button"
        className="delete"
        onClick={() => onDelete(task.id)}
        aria-label="Удалить"
      >
        ×
      </button>
    </li>
  );
}

function App() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [draft, setDraft] = useState("");
  const [loadError, setLoadError] = useState<string | null>(null);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );

  const { activeTasks, doneTasks } = useMemo(() => {
    const active = tasks.filter((t) => !t.done);
    const done = tasks.filter((t) => t.done);
    return { activeTasks: active, doneTasks: done };
  }, [tasks]);

  const activeIds = useMemo(
    () => activeTasks.map((t) => t.id),
    [activeTasks],
  );

  const loadTasks = async () => {
    const loaded = await invoke<Task[]>("tasks_load");
    setTasks(orderTasks(loaded));
    setLoadError(null);
  };

  useEffect(() => {
    void loadTasks().catch((e) => setLoadError(String(e)));
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void listen("tasks-purged", async () => {
      try {
        if (!cancelled) await loadTasks();
      } catch (e) {
        if (!cancelled) setLoadError(String(e));
      }
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const addTask = async () => {
    const title = draft.trim();
    if (!title) return;
    try {
      const created = await invoke<Task>("task_create", { title });
      setTasks((prev) => orderTasks([...prev, created]));
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
      orderTasks(
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
        orderTasks(current.map((item) => (item.id === id ? updated : item))),
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

  const onDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;

    const activeId = Number(active.id);
    const overId = Number(over.id);
    const oldIndex = activeTasks.findIndex((t) => t.id === activeId);
    const newIndex = activeTasks.findIndex((t) => t.id === overId);
    if (oldIndex < 0 || newIndex < 0) return;

    const prev = tasks;
    const reorderedActive = arrayMove(activeTasks, oldIndex, newIndex);
    setTasks(orderTasks([...reorderedActive, ...doneTasks]));

    try {
      const { task: updated, rebalanced } = await invoke<TaskMoveResult>(
        "task_move_active",
        { id: activeId, newIndex },
      );
      if (rebalanced) {
        const loaded = await invoke<Task[]>("tasks_load");
        setTasks(orderTasks(loaded));
      } else {
        setTasks((current) =>
          orderTasks(
            current.map((item) => (item.id === activeId ? updated : item)),
          ),
        );
      }
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

      <div className="task-list">
        <DndContext
          sensors={sensors}
          collisionDetection={closestCenter}
          onDragEnd={(e) => void onDragEnd(e)}
        >
          <SortableContext
            items={activeIds}
            strategy={verticalListSortingStrategy}
          >
            <ul className="task-section">
              {activeTasks.map((t) => (
                <SortableActiveRow
                  key={t.id}
                  task={t}
                  onToggle={toggleTask}
                  onDelete={removeTask}
                />
              ))}
            </ul>
          </SortableContext>
          <ul className="task-section">
            {doneTasks.map((t) => (
              <DoneRow
                key={t.id}
                task={t}
                onToggle={toggleTask}
                onDelete={removeTask}
              />
            ))}
          </ul>
        </DndContext>
      </div>
    </div>
  );
}

export default App;
