// Shapes mirrored from the wipe-daemon REST API.

export interface Comment {
  id: string;
  author: string;
  body: string;
  created: string;
}

export interface Ticket {
  id: string;
  title: string;
  body?: string;
  type?: string;
  priority?: string;
  labels: string[];
  tags: string[];
  assignees: string[];
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

export interface CreateTicketInput {
  title: string;
  type?: string;
  priority?: string;
  list?: string;
  body?: string;
  labels?: string[];
  tags?: string[];
  assignees?: string[];
}
