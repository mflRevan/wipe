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
    name: "init",
    summary: "Create a new board in the current project.",
    commands: [
      {
        command: "wipe init",
        description: "Initialize a .wipe/ board at the given path.",
        example: "wipe init .",
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
    name: "ticket",
    summary: "Create, move, inspect, list, and close tickets.",
    commands: [
      {
        command: "wipe ticket create",
        description: "Create a ticket with a title, type, and priority.",
        example:
          'wipe ticket create --title "Add login" --type feature --priority high --json',
      },
      {
        command: "wipe ticket move",
        description: "Move a ticket to another list.",
        example: "wipe ticket move T-1 --to in-progress",
      },
      {
        command: "wipe ticket show",
        description: "Show a single ticket with all of its fields and comments.",
        example: "wipe ticket show T-1 --json",
      },
      {
        command: "wipe ticket list",
        description: "List tickets, optionally filtered by list.",
        example: "wipe ticket list --list todo --json",
      },
      {
        command: "wipe ticket close",
        description: "Close a ticket.",
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
    summary: "Define and assign colored labels.",
    commands: [
      {
        command: "wipe label create",
        description: "Create a label with a name and hex color.",
        example: 'wipe label create needs-review --color "#f5a623"',
      },
      {
        command: "wipe label assign",
        description: "Assign an existing label to a ticket.",
        example: "wipe label assign T-1 needs-review",
      },
    ],
  },
  {
    name: "tag",
    summary: "Attach freeform tags to tickets.",
    commands: [
      {
        command: "wipe tag add",
        description: "Add a tag to a ticket.",
        example: "wipe tag add T-1 backend",
      },
    ],
  },
  {
    name: "config",
    summary: "Read and write board settings.",
    commands: [
      {
        command: "wipe config set",
        description: "Set a configuration value, e.g. the daemon port.",
        example: "wipe config set daemon.port 6737",
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
          "Start the local daemon and open the drag-and-drop desktop app.",
        example: "wipe serve",
      },
    ],
  },
  {
    name: "doctor",
    summary: "Diagnose board and environment issues.",
    commands: [
      {
        command: "wipe doctor",
        description: "Check the board's integrity and report any problems.",
        example: "wipe doctor",
      },
    ],
  },
  {
    name: "skill",
    summary: "Print the agent guide.",
    commands: [
      {
        command: "wipe skill",
        description:
          "Print a self-contained guide teaching an agent how to drive wipe.",
        example: "wipe skill",
      },
    ],
  },
];
