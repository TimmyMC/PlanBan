// Mirrors clabby_core::overview::OverviewRow (kept tracker-neutral, §11).

export type GitState = {
  branch: string | null;
  ahead: number;
  behind: number;
  dirty: boolean;
  lastCommitSummary: string | null;
};

export type SessionLite = {
  id: number;
  kind: "managed" | "external";
  status: string;
};

export type OverviewRow = {
  key: string;
  summary: string;
  /** The status Clabby believes / the column the card sits in. */
  status: string;
  /** Status last seen in the tracker (for the divergence badge). */
  trackerStatus: string;
  diverged: boolean;
  sessions: SessionLite[];
  worktreePath: string | null;
  git: GitState | null;
};

export type BoardData = {
  statuses: string[];
  rows: OverviewRow[];
};

/** Result of a gated transition (mirrors engine::TransitionOutcome). */
export type MoveResult = {
  completed: boolean;
  blocked?: { stepId: string; error: string };
};
