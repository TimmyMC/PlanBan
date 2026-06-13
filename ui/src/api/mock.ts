import type { Api } from "@/api";
import type { BoardData, MoveResult, OverviewRow } from "@/types";

const STATUSES = ["To Do", "In Progress", "In Review", "Done"];

let rows: OverviewRow[] = [
  {
    key: "PROJ-12",
    summary: "Implement tracker sync",
    status: "In Progress",
    trackerStatus: "In Progress",
    diverged: false,
    sessions: [{ id: 1, kind: "managed", status: "running" }],
    worktreePath: "wt/PROJ-12",
    git: { branch: "feature/PROJ-12", ahead: 3, behind: 0, dirty: true, lastCommitSummary: "wip: sync" },
  },
  {
    key: "PROJ-31",
    summary: "Review the worktree PR",
    status: "In Review",
    trackerStatus: "In Review",
    diverged: false,
    sessions: [{ id: 2, kind: "external", status: "idle" }],
    worktreePath: "wt/PROJ-31",
    git: { branch: "feature/PROJ-31", ahead: 0, behind: 0, dirty: false, lastCommitSummary: "ready" },
  },
  {
    key: "PROJ-44",
    summary: "Fix flaky integration test",
    status: "In Progress",
    trackerStatus: "Done",
    diverged: true,
    sessions: [],
    worktreePath: null,
    git: null,
  },
  {
    key: "PROJ-58",
    summary: "Add dark mode toggle",
    status: "To Do",
    trackerStatus: "To Do",
    diverged: false,
    sessions: [],
    worktreePath: null,
    git: null,
  },
];

function clone(): OverviewRow[] {
  return JSON.parse(JSON.stringify(rows));
}

export const mockApi: Api = {
  async getBoard(): Promise<BoardData> {
    return { statuses: STATUSES, rows: clone() };
  },
  async sync() {
    return { fetched: rows.length, diverged: rows.filter((r) => r.diverged).length };
  },
  async move(key, to): Promise<MoveResult> {
    const row = rows.find((r) => r.key === key);
    if (!row) throw new Error(`no such issue: ${key}`);
    // Demonstrate the gate: In Review -> Done requires an approval that fails.
    if (row.status === "In Review" && to === "Done") {
      return { completed: false, blocked: { stepId: "require_review", error: "PR is not approved yet" } };
    }
    row.status = to;
    row.diverged = false;
    return { completed: true };
  },
  async override(key): Promise<MoveResult> {
    const row = rows.find((r) => r.key === key);
    if (row) {
      row.status = "Done";
      row.diverged = false;
    }
    return { completed: true };
  },
};
