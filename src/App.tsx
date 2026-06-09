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
import { useEffect, useMemo, useRef, useState } from "react";
import { getDeleteConfirmPosition } from "./lib/deleteConfirmPosition";
import { orderTasks, type Task } from "./lib/orderTasks";
import "./App.css";

type DeleteConfirmState = {
  taskId: number;
  anchor: DOMRect;
};

function DeleteConfirmPopover({
  anchor,
  onConfirm,
  onCancel,
}: {
  anchor: DOMRect;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState(() =>
    getDeleteConfirmPosition(
      anchor,
      168,
      72,
      window.innerWidth,
      window.innerHeight,
    ),
  );

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    setPos(
      getDeleteConfirmPosition(
        anchor,
        width,
        height,
        window.innerWidth,
        window.innerHeight,
      ),
    );
  }, [anchor]);

  return (
    <div
      ref={ref}
      className="delete-confirm"
      role="dialog"
      aria-labelledby="delete-confirm-text"
      data-placement={pos.placement}
      style={
        {
          top: pos.top,
          left: pos.left,
          "--tail-top": `${pos.tailTop}px`,
        } as React.CSSProperties
      }
    >
      <p id="delete-confirm-text" className="delete-confirm-text">
        Вы уверены, что хотите удалить задачу?
      </p>
      <div className="delete-confirm-actions">
        <button type="button" className="delete-confirm-no" onClick={onCancel}>
          Отмена
        </button>
        <button
          type="button"
          className="delete-confirm-yes"
          onClick={onConfirm}
        >
          Да
        </button>
      </div>
    </div>
  );
}

type TaskMoveResult = {
  task: Task;
  rebalanced: boolean;
};

type TaskRowEditProps = {
  task: Task;
  isEditing: boolean;
  editDraft: string;
  onEditDraftChange: (value: string) => void;
  onEditKeyDown: (e: React.KeyboardEvent, taskId: number) => void;
  onEditClick: (task: Task) => void;
};

function TaskEditZone({
  task,
  isEditing,
  editDraft,
  onEditDraftChange,
  onEditKeyDown,
  onEditClick,
  inputRef,
}: TaskRowEditProps & { inputRef: React.RefObject<HTMLInputElement | null> }) {
  useEffect(() => {
    if (isEditing && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [isEditing, inputRef]);

  return (
    <div className="task-edit-zone">
      {isEditing ? (
        <input
          ref={inputRef}
          type="text"
          className="task-title-input"
          value={editDraft}
          onChange={(e) => onEditDraftChange(e.target.value)}
          onKeyDown={(e) => onEditKeyDown(e, task.id)}
          onPointerDown={(e) => e.stopPropagation()}
          onClick={(e) => e.stopPropagation()}
        />
      ) : (
        <span className="task-title">{task.title}</span>
      )}
      <button
        type="button"
        className="edit"
        onPointerDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          onEditClick(task);
        }}
        aria-label="Редактировать"
      >
        ✎
      </button>
    </div>
  );
}

function SortableActiveRow({
  task,
  isEditing,
  editDraft,
  onToggle,
  onDeleteRequest,
  onEditDraftChange,
  onEditKeyDown,
  onEditClick,
  inputRef,
}: TaskRowEditProps & {
  onToggle: (id: number) => void;
  onDeleteRequest: (id: number, button: HTMLButtonElement) => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
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

  const className = isDragging
    ? "task task-dragging"
    : isEditing
      ? "task task-editing"
      : "task";

  return (
    <li
      ref={setNodeRef}
      style={style}
      className={className}
      {...attributes}
      {...(isEditing ? {} : listeners)}
    >
      <input
        type="checkbox"
        checked={task.done}
        onPointerDown={(e) => e.stopPropagation()}
        onClick={(e) => e.stopPropagation()}
        onChange={() => onToggle(task.id)}
      />
      <TaskEditZone
        task={task}
        isEditing={isEditing}
        editDraft={editDraft}
        onEditDraftChange={onEditDraftChange}
        onEditKeyDown={onEditKeyDown}
        onEditClick={onEditClick}
        inputRef={inputRef}
      />
      <button
        type="button"
        className="delete"
        onPointerDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          onDeleteRequest(task.id, e.currentTarget);
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
  isEditing,
  editDraft,
  onToggle,
  onDeleteRequest,
  onEditDraftChange,
  onEditKeyDown,
  onEditClick,
  inputRef,
}: TaskRowEditProps & {
  onToggle: (id: number) => void;
  onDeleteRequest: (id: number, button: HTMLButtonElement) => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
}) {
  return (
    <li className={isEditing ? "task done task-editing" : "task done"}>
      <input
        type="checkbox"
        checked={task.done}
        onChange={() => onToggle(task.id)}
      />
      <TaskEditZone
        task={task}
        isEditing={isEditing}
        editDraft={editDraft}
        onEditDraftChange={onEditDraftChange}
        onEditKeyDown={onEditKeyDown}
        onEditClick={onEditClick}
        inputRef={inputRef}
      />
      <button
        type="button"
        className="delete"
        onClick={(e) => onDeleteRequest(task.id, e.currentTarget)}
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
  const [deleteConfirm, setDeleteConfirm] = useState<DeleteConfirmState | null>(
    null,
  );
  const [editingTaskId, setEditingTaskId] = useState<number | null>(null);
  const [editDraft, setEditDraft] = useState("");
  const deleteConfirmRef = useRef<HTMLDivElement>(null);
  const editInputRef = useRef<HTMLInputElement>(null);

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

  const cancelEdit = () => {
    setEditingTaskId(null);
    setEditDraft("");
  };

  const startEdit = (task: Task) => {
    setDeleteConfirm(null);
    setEditingTaskId(task.id);
    setEditDraft(task.title);
  };

  const commitEdit = async (id: number) => {
    const task = tasks.find((t) => t.id === id);
    if (!task) {
      cancelEdit();
      return;
    }
    const title = editDraft.trim();
    if (!title || title === task.title) {
      cancelEdit();
      return;
    }
    const prev = tasks;
    const draftForRestore = editDraft;
    setEditingTaskId(null);
    setEditDraft("");
    setTasks(
      orderTasks(
        tasks.map((item) => (item.id === id ? { ...item, title } : item)),
      ),
    );
    try {
      const updated = await invoke<Task>("task_update_title", { id, title });
      setTasks((current) =>
        orderTasks(current.map((item) => (item.id === id ? updated : item))),
      );
      setLoadError(null);
    } catch (e) {
      setTasks(prev);
      setEditingTaskId(id);
      setEditDraft(draftForRestore);
      setLoadError(String(e));
    }
  };

  const toggleEdit = (task: Task) => {
    if (editingTaskId === task.id) {
      void commitEdit(task.id);
    } else {
      startEdit(task);
    }
  };

  const handleEditKeyDown = (e: React.KeyboardEvent, taskId: number) => {
    if (e.key === "Enter") {
      e.preventDefault();
      void commitEdit(taskId);
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancelEdit();
    }
  };

  useEffect(() => {
    if (editingTaskId === null) return;

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") cancelEdit();
    };

    const onPointerDown = (e: PointerEvent) => {
      const target = e.target as Element;
      if (target.closest?.(".task-edit-zone")) return;
      if (deleteConfirmRef.current?.contains(target)) return;
      if (target.closest?.(".delete")) {
        cancelEdit();
        return;
      }
      void commitEdit(editingTaskId);
    };

    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [editingTaskId, editDraft, tasks]);

  useEffect(() => {
    if (!deleteConfirm) return;

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setDeleteConfirm(null);
    };

    const onPointerDown = (e: PointerEvent) => {
      const target = e.target as Node;
      if (deleteConfirmRef.current?.contains(target)) return;
      if ((target as Element).closest?.(".delete")) return;
      setDeleteConfirm(null);
    };

    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [deleteConfirm]);

  const requestDelete = (id: number, button: HTMLButtonElement) => {
    if (deleteConfirm?.taskId === id) {
      setDeleteConfirm(null);
      return;
    }
    setDeleteConfirm({ taskId: id, anchor: button.getBoundingClientRect() });
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
        <button
          type="button"
          className="hint"
          data-tauri-drag-region={false}
          title="Скрыть или показать панель"
          onClick={() => void invoke("panel_toggle")}
        >
          Ctrl+Shift+Space
        </button>
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
                  isEditing={editingTaskId === t.id}
                  editDraft={editDraft}
                  onToggle={toggleTask}
                  onDeleteRequest={requestDelete}
                  onEditDraftChange={setEditDraft}
                  onEditKeyDown={handleEditKeyDown}
                  onEditClick={toggleEdit}
                  inputRef={editInputRef}
                />
              ))}
            </ul>
          </SortableContext>
          <ul className="task-section">
            {doneTasks.map((t) => (
              <DoneRow
                key={t.id}
                task={t}
                isEditing={editingTaskId === t.id}
                editDraft={editDraft}
                onToggle={toggleTask}
                onDeleteRequest={requestDelete}
                onEditDraftChange={setEditDraft}
                onEditKeyDown={handleEditKeyDown}
                onEditClick={toggleEdit}
                inputRef={editInputRef}
              />
            ))}
          </ul>
        </DndContext>
      </div>

      {deleteConfirm ? (
        <div ref={deleteConfirmRef}>
          <DeleteConfirmPopover
            anchor={deleteConfirm.anchor}
            onCancel={() => setDeleteConfirm(null)}
            onConfirm={() => {
              const id = deleteConfirm.taskId;
              setDeleteConfirm(null);
              void removeTask(id);
            }}
          />
        </div>
      ) : null}
    </div>
  );
}

export default App;
