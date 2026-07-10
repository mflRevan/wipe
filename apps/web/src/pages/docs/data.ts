export interface DocSection {
  slug: string;
  title: string;
}

export const DOC_SECTIONS: DocSection[] = [
  { slug: "introduction", title: "Introduction" },
  { slug: "installation", title: "Installation" },
  { slug: "quickstart", title: "Quickstart" },
  { slug: "concepts", title: "Core concepts" },
  { slug: "cli", title: "CLI reference" },
  { slug: "agents", title: "For agents" },
  { slug: "humans", title: "For humans" },
];

export interface CliCommand {
  command: string;
  description: string;
  example: string;
}

export interface CliGroup {
  name: string;
  summary: string;
  commands: CliCommand[];
}

export const CLI_GROUPS: CliGroup[] = [
  {
    name: "onboard",
    summary:
      "One-time, machine-wide guided setup. Offered automatically the first time you run wipe.",
    commands: [
      {
        command: "wipe onboard",
        description:
          "Walk through your global defaults: identity, UI port, exposure, login autostart, board starter, agent skill, and styling. Writes your user config (not any board).",
        example: "wipe onboard",
      },
    ],
  },
  {
    name: "init",
    summary: "Create a new board in the current project.",
    commands: [
      {
        command: "wipe init",
        description:
          "Initialize a .wipe/ board with a short wizard (name, starter, UI port, skill). Add --yes to skip it and use your global defaults.",
        example: "wipe init .",
      },
    ],
  },
  {
    name: "identity",
    summary: "See and manage who your actions are attributed to (humans and agents).",
    commands: [
      {
        command: "wipe identity whoami",
        description: "Show the identity actions are currently attributed to, and why.",
        example: "wipe identity whoami",
      },
      {
        command: "wipe identity list",
        description:
          "List identities from the board registry and your version control, marking the active one. Agents: run this first.",
        example: "wipe identity list --json",
      },
      {
        command: "wipe identity use",
        description:
          "Bind an identity to this terminal session (creating it if new). Use --agent for an agent identity.",
        example: "wipe identity use planner-bot --agent",
      },
    ],
  },
  {
    name: "status",
    summary: "Show the board at a glance.",
    commands: [
      {
        command: "wipe status",
        description:
          "Print every list and its tickets. Add --json for machine-readable output.",
        example: "wipe status --json",
      },
    ],
  },
  {
    name: "board",
    summary: "Inspect and rename the board itself.",
    commands: [
      {
        command: "wipe board show",
        description: "Show board metadata (name, id, list count).",
        example: "wipe board show --json",
      },
      {
        command: "wipe board rename",
        description: "Rename the board.",
        example: 'wipe board rename "Platform Team"',
      },
    ],
  },
  {
    name: "list",
    summary: "Manage the board's columns.",
    commands: [
      {
        command: "wipe list add",
        description: "Add a new list to the end of the board.",
        example: 'wipe list add "In Review"',
      },
      {
        command: "wipe list move",
        description: "Reorder a list to a new 0-based position.",
        example: "wipe list move in-review 2",
      },
      {
        command: "wipe list remove",
        description: "Remove a list (use --force to also delete its tickets).",
        example: "wipe list remove in-review",
      },
    ],
  },
  {
    name: "ticket",
    summary: "Create, move, edit, inspect, assign, list, and close tickets.",
    commands: [
      {
        command: "wipe ticket create",
        description: "Create a ticket with a title, priority, labels, and assignees.",
        example: 'wipe ticket create --title "Add login" --priority high --json',
      },
      {
        command: "wipe ticket move",
        description: "Move a ticket to another list (optionally at a position).",
        example: "wipe ticket move T-1 --to in-progress",
      },
      {
        command: "wipe ticket edit",
        description: "Edit a ticket's title, body, or priority.",
        example: 'wipe ticket edit T-1 --priority urgent',
      },
      {
        command: "wipe ticket assign",
        description: "Add or remove an assignee (a human or an agent identity).",
        example: "wipe ticket assign T-1 planner-bot",
      },
      {
        command: "wipe ticket show",
        description: "Show a single ticket with all of its fields and comments.",
        example: "wipe ticket show T-1 --json",
      },
      {
        command: "wipe ticket list",
        description: "List tickets, optionally filtered by list or label.",
        example: "wipe ticket list --list todo --json",
      },
      {
        command: "wipe ticket close",
        description: "Move a ticket to the done list.",
        example: "wipe ticket close T-1",
      },
    ],
  },
  {
    name: "comment",
    summary: "Discuss work inline on a ticket.",
    commands: [
      {
        command: "wipe comment add",
        description: "Append a comment to a ticket.",
        example: 'wipe comment add T-1 --body "Blocked on the API design."',
      },
      {
        command: "wipe comment list",
        description: "List all comments on a ticket.",
        example: "wipe comment list T-1",
      },
    ],
  },
  {
    name: "label",
    summary: "Define and assign colored labels (the board's categorization).",
    commands: [
      {
        command: "wipe label create",
        description: "Create a label; a color is auto-assigned if you don't pass one.",
        example: "wipe label create needs-review",
      },
      {
        command: "wipe label assign",
        description: "Assign an existing label to a ticket.",
        example: "wipe label assign T-1 needs-review",
      },
      {
        command: "wipe label list",
        description: "List every defined label.",
        example: "wipe label list --json",
      },
    ],
  },
  {
    name: "media",
    summary: "Attach version-controlled files to tickets.",
    commands: [
      {
        command: "wipe media add",
        description: "Attach a file to a ticket (copied into .wipe/media/ and tracked in git).",
        example: "wipe media add T-1 ./design.png",
      },
      {
        command: "wipe media list",
        description: "List a ticket's attachments.",
        example: "wipe media list T-1",
      },
    ],
  },
  {
    name: "forum",
    summary:
      "Git-tracked discussion threads where humans and agents record decisions and gotchas.",
    commands: [
      {
        command: "wipe forum post",
        description: "Open a new thread with a root post.",
        example: 'wipe forum post --title "Rework auth" --body "Proposal inside."',
      },
      {
        command: "wipe forum reply",
        description: "Reply to a post at any depth (parent is a post id like F-1 or F-1.2).",
        example: 'wipe forum reply F-1 --body "Agreed - let\'s do it."',
      },
      {
        command: "wipe forum search",
        description: "Search posts by regex and/or filters (author, label, thread).",
        example: 'wipe forum search "timeout" --json',
      },
      {
        command: "wipe forum watch",
        description:
          "Stream new matching posts as newline-delimited JSON - agent harnesses react to each line.",
        example: "wipe forum watch --replay",
      },
    ],
  },
  {
    name: "scan",
    summary: "Discover boards across your machine so the UI can list them.",
    commands: [
      {
        command: "wipe scan",
        description:
          "Walk the given paths (or your configured scan roots) for .wipe boards and register them.",
        example: "wipe scan ~/code",
      },
    ],
  },
  {
    name: "serve",
    summary: "Launch the local desktop UI.",
    commands: [
      {
        command: "wipe serve",
        description:
          "Start the local daemon and open the drag-and-drop desktop app for every board on this machine.",
        example: "wipe serve --open",
      },
    ],
  },
  {
    name: "config",
    summary: "Read and write settings - the board's, or your machine-wide defaults.",
    commands: [
      {
        command: "wipe config show",
        description: "Show this board's settings (add --global for your user defaults).",
        example: "wipe config show",
      },
      {
        command: "wipe config set",
        description: "Set a board setting, e.g. the daemon port.",
        example: "wipe config set daemon.port 6737",
      },
      {
        command: "wipe config --global set",
        description: "Set a machine-wide default that applies to every board.",
        example: "wipe config --global set default.port 6737",
      },
    ],
  },
  {
    name: "doctor",
    summary: "Diagnose board and environment issues.",
    commands: [
      {
        command: "wipe doctor",
        description: "Check the environment and the board's integrity, and report any problems.",
        example: "wipe doctor",
      },
    ],
  },
  {
    name: "skill",
    summary: "Print or install the agent guide.",
    commands: [
      {
        command: "wipe skill",
        description:
          "Print a self-contained guide teaching an agent how to drive wipe.",
        example: "wipe skill",
      },
      {
        command: "wipe skill install",
        description:
          "Install SKILL.md into an agent skills directory (.claude/skills or .agents/skills).",
        example: "wipe skill install --target claude",
      },
    ],
  },
];
