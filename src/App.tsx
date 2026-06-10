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
import { invokeWhenBackendReady } from "./lib/invokeWhenBackendReady";
import { useEffect, useMemo, useRef, useState } from "react";
import { getDeleteConfirmPosition } from "./lib/deleteConfirmPosition";
import { orderTasks, type Task, type TaskList } from "./lib/orderTasks";
import { type AppSettings } from "./lib/appSettings";
import { formatTaskDateTime } from "./lib/formatTaskDateTime";
import { formatHotkeyLabel } from "./lib/formatHotkeyLabel";
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
};

function TaskEditZone({
  task,
  isEditing,
  editDraft,
  onEditDraftChange,
  onEditKeyDown,
  inputRef,
  showCreatedAt,
}: TaskRowEditProps & {
  inputRef: React.RefObject<HTMLInputElement | null>;
  showCreatedAt: boolean;
}) {
  useEffect(() => {
    if (isEditing && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [isEditing, inputRef]);

  const withDateLayout = showCreatedAt && !isEditing;

  return (
    <div
      className={
        withDateLayout
          ? "task-edit-zone task-edit-zone--with-date"
          : "task-edit-zone"
      }
    >
      <div className="task-text-block">
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
          <>
            <span className="task-title">{task.title}</span>
            {showCreatedAt ? (
              <span className="task-created-at">
                {formatTaskDateTime(task.created_at)}
              </span>
            ) : null}
          </>
        )}
      </div>
    </div>
  );
}

function TaskActions({
  showBacklogAction,
  backlogActionIcon,
  backlogActionLabel,
  onBacklogAction,
  onEditClick,
  onDeleteRequest,
  taskId,
  stopPropagation,
}: {
  showBacklogAction: boolean;
  backlogActionIcon: string;
  backlogActionLabel: string;
  onBacklogAction: (id: number) => void;
  onEditClick: () => void;
  onDeleteRequest: (id: number, button: HTMLButtonElement) => void;
  taskId: number;
  stopPropagation: boolean;
}) {
  const stop = stopPropagation
    ? {
        onPointerDown: (e: React.PointerEvent) => e.stopPropagation(),
        onClick: (e: React.MouseEvent) => e.stopPropagation(),
      }
    : {};

  return (
    <div className="task-actions">
      <button
        type="button"
        className="task-action-btn task-action-btn--edit"
        aria-label="Редактировать"
        {...stop}
        onClick={(e) => {
          if (stopPropagation) e.stopPropagation();
          onEditClick();
        }}
      >
        ✎
      </button>
      {showBacklogAction ? (
        <button
          type="button"
          className="task-action-btn task-action-btn--backlog"
          aria-label={backlogActionLabel}
          {...stop}
          onClick={(e) => {
            if (stopPropagation) e.stopPropagation();
            onBacklogAction(taskId);
          }}
        >
          {backlogActionIcon}
        </button>
      ) : null}
      <button
        type="button"
        className="task-action-btn task-action-btn--delete"
        aria-label="Удалить"
        {...(stopPropagation ? { onPointerDown: stop.onPointerDown } : {})}
        onClick={(e) => {
          if (stopPropagation) e.stopPropagation();
          onDeleteRequest(taskId, e.currentTarget);
        }}
      >
        ×
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
  onBacklogAction,
  inputRef,
  showCreatedAt,
  showBacklogAction,
  backlogActionIcon,
  backlogActionLabel,
}: TaskRowEditProps & {
  onToggle: (id: number) => void;
  onDeleteRequest: (id: number, button: HTMLButtonElement) => void;
  onEditClick: (task: Task) => void;
  onBacklogAction: (id: number) => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
  showCreatedAt: boolean;
  showBacklogAction: boolean;
  backlogActionIcon: string;
  backlogActionLabel: string;
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
        inputRef={inputRef}
        showCreatedAt={showCreatedAt}
      />
      <TaskActions
        showBacklogAction={showBacklogAction}
        backlogActionIcon={backlogActionIcon}
        backlogActionLabel={backlogActionLabel}
        onBacklogAction={onBacklogAction}
        onEditClick={() => onEditClick(task)}
        onDeleteRequest={onDeleteRequest}
        taskId={task.id}
        stopPropagation
      />
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
  onBacklogAction,
  inputRef,
  showCreatedAt,
  showBacklogAction,
  backlogActionIcon,
  backlogActionLabel,
}: TaskRowEditProps & {
  onToggle: (id: number) => void;
  onDeleteRequest: (id: number, button: HTMLButtonElement) => void;
  onEditClick: (task: Task) => void;
  onBacklogAction: (id: number) => void;
  inputRef: React.RefObject<HTMLInputElement | null>;
  showCreatedAt: boolean;
  showBacklogAction: boolean;
  backlogActionIcon: string;
  backlogActionLabel: string;
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
        inputRef={inputRef}
        showCreatedAt={showCreatedAt}
      />
      <TaskActions
        showBacklogAction={showBacklogAction}
        backlogActionIcon={backlogActionIcon}
        backlogActionLabel={backlogActionLabel}
        onBacklogAction={onBacklogAction}
        onEditClick={() => onEditClick(task)}
        onDeleteRequest={onDeleteRequest}
        taskId={task.id}
        stopPropagation={false}
      />
    </li>
  );
}

const URGENT_TAB = { id: "urgent" as const, label: "Задачи" };
const DAILY_TAB = { id: "daily" as const, label: "Ежедневные" };
const WEEKLY_TAB = { id: "weekly" as const, label: "Еженедельные" };
const BACKLOG_TAB = { id: "backlog" as const, label: "Бэклог" };

function applyUiSettings(settings: AppSettings) {
  return {
    hotkeyLabel: formatHotkeyLabel(settings.hotkey),
    showCreatedAt: settings.show_created_at,
    showCompletedTasks: settings.show_completed_tasks,
    confirmTaskDelete: settings.confirm_task_delete,
    useDaily: settings.use_daily,
    useWeekly: settings.use_weekly,
    useBacklog: settings.use_backlog,
  };
}

function App() {
  const [activeList, setActiveList] = useState<TaskList>("urgent");
  const [tasks, setTasks] = useState<Task[]>([]);
  const [draft, setDraft] = useState("");
  const [loadError, setLoadError] = useState<string | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<DeleteConfirmState | null>(
    null,
  );
  const [editingTaskId, setEditingTaskId] = useState<number | null>(null);
  const [editDraft, setEditDraft] = useState("");
  const [hotkeyLabel, setHotkeyLabel] = useState("Ctrl+Shift+Space");
  const [showCreatedAt, setShowCreatedAt] = useState(true);
  const [showCompletedTasks, setShowCompletedTasks] = useState(true);
  const [confirmTaskDelete, setConfirmTaskDelete] = useState(true);
  const [useDaily, setUseDaily] = useState(true);
  const [useWeekly, setUseWeekly] = useState(true);
  const [useBacklog, setUseBacklog] = useState(true);
  const deleteConfirmRef = useRef<HTMLDivElement>(null);
  const editInputRef = useRef<HTMLInputElement>(null);
  const activeListRef = useRef<TaskList>(activeList);
  activeListRef.current = activeList;

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

  const loadTasks = async (list: TaskList = activeList) => {
    const loaded = await invokeWhenBackendReady<Task[]>("tasks_load", { list });
    setTasks(orderTasks(loaded));
    setLoadError(null);
  };

  useEffect(() => {
    setEditingTaskId(null);
    setEditDraft("");
    setDeleteConfirm(null);
    void loadTasks(activeList).catch((e) => setLoadError(String(e)));
  }, [activeList]);

  useEffect(() => {
    void invokeWhenBackendReady<AppSettings>("settings_load")
      .then((settings) => {
        const ui = applyUiSettings(settings);
        setHotkeyLabel(ui.hotkeyLabel);
        setShowCreatedAt(ui.showCreatedAt);
        setShowCompletedTasks(ui.showCompletedTasks);
        setConfirmTaskDelete(ui.confirmTaskDelete);
        setUseDaily(ui.useDaily);
        setUseWeekly(ui.useWeekly);
        setUseBacklog(ui.useBacklog);
      })
      .catch((e) => setLoadError(String(e)));
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void listen<AppSettings>("settings-changed", (event) => {
      const ui = applyUiSettings(event.payload);
      setHotkeyLabel(ui.hotkeyLabel);
      setShowCreatedAt(ui.showCreatedAt);
      setShowCompletedTasks(ui.showCompletedTasks);
      setConfirmTaskDelete(ui.confirmTaskDelete);
      setUseDaily(ui.useDaily);
      setUseWeekly(ui.useWeekly);
      setUseBacklog(ui.useBacklog);
      const current = activeListRef.current;
      if (
        (!ui.useBacklog && current === "backlog") ||
        (!ui.useDaily && current === "daily") ||
        (!ui.useWeekly && current === "weekly")
      ) {
        setActiveList("urgent");
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

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void listen("tasks-purged", async () => {
      try {
        if (!cancelled) await loadTasks(activeList);
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
  }, [activeList]);

  const showCreatedAtForList =
    showCreatedAt && (activeList === "urgent" || activeList === "backlog");

  const visibleTabs = useMemo(() => {
    const tabs: { id: TaskList; label: string }[] = [URGENT_TAB];
    if (useDaily) tabs.push(DAILY_TAB);
    if (useWeekly) tabs.push(WEEKLY_TAB);
    if (useBacklog) tabs.push(BACKLOG_TAB);
    return tabs;
  }, [useDaily, useWeekly, useBacklog]);

  const showBacklogActions =
    useBacklog && (activeList === "urgent" || activeList === "backlog");
  const backlogActionIcon = activeList === "urgent" ? "⤵" : "↩";
  const backlogActionLabel =
    activeList === "urgent" ? "В бэклог" : "Вернуть в задачи";

  const addTask = async () => {
    const title = draft.trim();
    if (!title) return;
    try {
      const created = await invoke<Task>("task_create", {
        title,
        list: activeList,
      });
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
      if (target.closest?.(".task-actions")) return;
      if (deleteConfirmRef.current?.contains(target)) return;
      if (target.closest?.(".task-action-btn--delete")) {
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
      if ((target as Element).closest?.(".task-action-btn--delete")) return;
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
    if (!confirmTaskDelete) {
      void removeTask(id);
      return;
    }
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

  const moveTaskList = async (id: number) => {
    const targetList: TaskList = activeList === "urgent" ? "backlog" : "urgent";
    const prev = tasks;
    setTasks((t) => t.filter((item) => item.id !== id));
    try {
      await invoke<Task>("task_move_list", { id, list: targetList });
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
        const loaded = await invoke<Task[]>("tasks_load", { list: activeList });
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
        <span className="title">Focus Task Tracker</span>
        <div className="titlebar-actions" data-tauri-drag-region={false}>
          <button
            type="button"
            className="settings-btn"
            title="Настройки"
            aria-label="Настройки"
            onClick={() =>
              void invoke("settings_open").catch((e) => setLoadError(String(e)))
            }
          >
            ⚙
          </button>
          <button
            type="button"
            className="hint"
            title="Скрыть или показать панель"
            onClick={() => void invoke("panel_toggle")}
          >
            {hotkeyLabel}
          </button>
        </div>
      </header>

      {loadError ? <p className="error">{loadError}</p> : null}

      <nav className="tab-bar" aria-label="Списки задач">
        {visibleTabs.map((tab, index) => {
          const isActive = activeList === tab.id;
          const isFirst = index === 0;
          const isLast = index === visibleTabs.length - 1;
          return (
            <div key={tab.id} className="tab-slot">
              <button
                type="button"
                className={[
                  "tab",
                  isActive && "tab-active",
                  isActive && isFirst && "tab-active-first",
                  isActive && isLast && "tab-active-last",
                ]
                  .filter(Boolean)
                  .join(" ")}
                aria-selected={isActive}
                onClick={() => setActiveList(tab.id)}
              >
                {tab.label}
              </button>
            </div>
          );
        })}
      </nav>

      <div className="app-body">
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
                  onBacklogAction={(id) => void moveTaskList(id)}
                  inputRef={editInputRef}
                  showCreatedAt={showCreatedAtForList}
                  showBacklogAction={showBacklogActions}
                  backlogActionIcon={backlogActionIcon}
                  backlogActionLabel={backlogActionLabel}
                />
              ))}
            </ul>
          </SortableContext>
          {showCompletedTasks ? (
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
                  onBacklogAction={(id) => void moveTaskList(id)}
                  inputRef={editInputRef}
                  showCreatedAt={showCreatedAtForList}
                  showBacklogAction={showBacklogActions}
                  backlogActionIcon={backlogActionIcon}
                  backlogActionLabel={backlogActionLabel}
                />
              ))}
            </ul>
          ) : null}
        </DndContext>
      </div>
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
