export type TaskList = "urgent" | "daily" | "weekly";

export type Task = {
  id: number;
  title: string;
  done: boolean;
  completed_at: string | null;
  position: number;
  created_at: string;
  list: TaskList;
};

export function orderTasks(tasks: Task[]): Task[] {
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
