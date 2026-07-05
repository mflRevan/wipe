// Shapes mirrored from the wipe-daemon REST API (see crates/wipe-core/src/model.rs).

export interface Comment {
  id: string;
  author: string;
  body: string;
  created: string;
  edited?: string;
}

/** A structured change event on a ticket (see wipe-core `Activity`). */
export interface Activity {
  ts: string;
  actor: string;
  /**
   * One of: created, moved, renamed, edited, priority, label-added,
   * label-removed, assigned, unassigned, attached, detached.
   */
  kind: string;
  detail?: string;
}

export type AttachmentSource = 'media' | 'repo';

export interface Attachment {
  name: string;
  path: string;
  source: AttachmentSource;
  size: number;
  mime: string;
}

export interface Ticket {
  version: number;
  id: string;
  title: string;
  body?: string;
  priority?: string;
  labels: string[];
  assignees: string[];
  attachments: Attachment[];
  comments: Comment[];
  activity: Activity[];
  created: string;
  updated: string;
}

export interface List {
  list: string;
  name: string;
  tickets: Ticket[];
}

export interface Board {
  board: string;
  commit?: string;
  lists: List[];
}

export interface Project {
  path: string;
  name: string;
}

export interface Health {
  ok: boolean;
  service: string;
  version: string;
}

/** User-global preferences surfaced by `GET/PATCH /api/config`. */
export interface AppConfig {
  accent?: string | null;
  theme?: string | null;
  default_identity?: string | null;
  prefer_default_identity?: boolean;
}

export interface CommitInfo {
  hash: string;
  short: string;
  author_name: string;
  author_email: string;
  date: string;
  subject: string;
}

/** A node in the repository-wide commit graph (`GET /api/graph`). */
export interface GraphCommit {
  hash: string;
  short: string;
  parents: string[];
  refs: string[];
  author_name: string;
  date: string;
  subject: string;
  /** True when this commit changed `.wipe/` - i.e. a board checkpoint. */
  board: boolean;
}

export interface LabelDef {
  name: string;
  color?: string;
  description?: string;
}

export interface Definitions {
  version?: number;
  labels: LabelDef[];
  priorities: string[];
}

/** A forum post: the root of a thread or a reply at any depth. */
export interface ForumPost {
  id: string;
  author: string;
  body: string;
  labels: string[];
  refs: string[];
  attachments: Attachment[];
  created: string;
  edited?: string;
  replies: ForumPost[];
}

export interface ForumThread {
  version: number;
  id: string;
  title: string;
  root: ForumPost;
  created: string;
  updated: string;
}

export interface ForumThreadSummary {
  id: string;
  title: string;
  author: string;
  labels: string[];
  posts: number;
  created: string;
}

/** A flattened post returned by forum search. */
export interface ForumMatch {
  id: string;
  thread_id: string;
  thread_title: string;
  author: string;
  body: string;
  labels: string[];
  refs: string[];
  depth: number;
  replies: number;
  attachments: number;
  created: string;
  edited?: string;
}

export type IdentityKind = 'human' | 'agent';

export interface Identity {
  id: string;
  display_name: string;
  kind: IdentityKind;
}

export interface CreateTicketInput {
  title: string;
  priority?: string;
  list?: string;
  body?: string;
  labels?: string[];
  assignees?: string[];
}

/**
 * Partial ticket update. Omit a key to leave it unchanged. For `priority`,
 * pass `null` to CLEAR the value (JSON.stringify keeps null, drops undefined -
 * which is exactly the daemon's Option<Option<String>> semantics).
 */
export interface TicketPatch {
  title?: string;
  body?: string;
  priority?: string | null;
  labels?: string[];
  assignees?: string[];
}
