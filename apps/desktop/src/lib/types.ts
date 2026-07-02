// Shapes mirrored from the wipe-daemon REST API (see crates/wipe-core/src/model.rs).

export interface Comment {
  id: string;
  author: string;
  body: string;
  created: string;
  edited?: string;
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
  /** True when this commit changed `.wipe/` — i.e. a board checkpoint. */
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
 * pass `null` to CLEAR the value (JSON.stringify keeps null, drops undefined —
 * which is exactly the daemon's Option<Option<String>> semantics).
 */
export interface TicketPatch {
  title?: string;
  body?: string;
  priority?: string | null;
  labels?: string[];
  assignees?: string[];
}
