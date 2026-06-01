import { describe, expect, it } from "vitest";
import { orderTasks, type Task } from "./orderTasks";

function task(partial: Partial<Task> & Pick<Task, "id" | "title" | "done">): Task {
  return {
    completed_at: null,
    position: partial.id,
    ...partial,
  };
}

describe("orderTasks", () => {
  it("puts active tasks above done", () => {
    const ordered = orderTasks([
      task({ id: 1, title: "done", done: true, completed_at: "2026-05-30" }),
      task({ id: 2, title: "active", done: false, position: 2 }),
    ]);
    expect(ordered[0].title).toBe("active");
    expect(ordered[1].title).toBe("done");
  });

  it("sorts active by position descending", () => {
    const ordered = orderTasks([
      task({ id: 1, title: "low", done: false, position: 1 }),
      task({ id: 2, title: "high", done: false, position: 10 }),
    ]);
    expect(ordered.map((t) => t.title)).toEqual(["high", "low"]);
  });

  it("sorts done by completed_at descending", () => {
    const ordered = orderTasks([
      task({
        id: 1,
        title: "old",
        done: true,
        completed_at: "2026-05-28 10:00:00",
      }),
      task({
        id: 2,
        title: "new",
        done: true,
        completed_at: "2026-05-30 10:00:00",
      }),
    ]);
    expect(ordered.map((t) => t.title)).toEqual(["new", "old"]);
  });
});
