import { Link, useParams } from "react-router-dom";
import { ArrowRight } from "lucide-react";
import { cn } from "@/lib/utils";
import { CodeBlock } from "@/components/CodeBlock";
import { CommandCard } from "@/components/CommandCard";
import { Button } from "@/components/ui/button";
import { Tabs } from "@/pages/docs/Tabs";
import { CLI_GROUPS, DOC_SECTIONS } from "@/pages/docs/data";

const WIPE_TREE = `.wipe/
  board.json         # lists + card order (ticket-id refs)
  definitions.json   # ticket types, labels, tags, priorities
  settings.json      # daemon port + exposure
  tickets/T-1.json   # one file per ticket (fields + inline comments)
  media/             # version-controlled attachments
  .cache/index.db    # gitignored, auto-managed`;

function Prose({ children }: { children: React.ReactNode }) {
  return <div className="space-y-4 text-[15px] leading-relaxed text-muted-foreground">{children}</div>;
}

function H2({ children, id }: { children: React.ReactNode; id?: string }) {
  return (
    <h2 id={id} className="scroll-mt-24 text-2xl font-bold tracking-tight text-foreground">
      {children}
    </h2>
  );
}

function H3({ children }: { children: React.ReactNode }) {
  return <h3 className="text-lg font-semibold text-foreground">{children}</h3>;
}

function Introduction() {
  return (
    <section className="space-y-6">
      <H2>Introduction</H2>
      <Prose>
        <p>
          <strong className="text-foreground">wipe</strong> is a CLI-first,
          git-native task board for collaboration between humans and AI agents —
          and agents with each other. There is no external service, no separate
          database, and no account to create: the board <em>is</em> a folder in
          your repo (<code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs text-primary">.wipe/</code>),
          and every change to it is a change you can diff, blame, branch, and
          merge like any other file.
        </p>
        <p>
          A board holds ordered <strong className="text-foreground">lists</strong>{" "}
          (Backlog, Todo, In Progress, Done…) containing{" "}
          <strong className="text-foreground">tickets</strong>. All state is
          stored as flat, deterministically formatted JSON designed for clean
          diffs and low-conflict merges, so two people — or two agents — can work
          on the same board on different branches and merge without a fight.
        </p>
        <p>
          Agents drive everything through the self-documenting{" "}
          <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs text-primary">wipe</code>{" "}
          CLI. Humans use a local desktop app (
          <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs text-primary">wipe serve</code>
          ) with drag-and-drop and a git-history board rewind.
        </p>
      </Prose>
    </section>
  );
}

function Installation() {
  return (
    <section className="space-y-6">
      <H2>Installation</H2>
      <Prose>
        <p>
          wipe is pre-1.0 and under active development. Install it through
          whichever channel suits you.
        </p>
      </Prose>
      <Tabs
        tabs={[
          {
            label: "npm",
            content: (
              <div className="space-y-3">
                <p className="text-sm text-muted-foreground">
                  A pure install wrapper that downloads the prebuilt binary.
                </p>
                <CodeBlock code="npm install -g wipe" prompt />
              </div>
            ),
          },
          {
            label: "cargo",
            content: (
              <div className="space-y-3">
                <p className="text-sm text-muted-foreground">
                  Build and install from source with Cargo.
                </p>
                <CodeBlock code="cargo install wipe" prompt />
              </div>
            ),
          },
          {
            label: "manual",
            content: (
              <div className="space-y-3">
                <p className="text-sm text-muted-foreground">
                  Clone the repository and build the workspace.
                </p>
                <CodeBlock
                  code={`git clone https://github.com/mflRevan/wipe.git\ncd wipe\ncargo build --workspace --release`}
                  prompt
                />
              </div>
            ),
          },
        ]}
      />
      <Prose>
        <p>
          Verify your install and environment at any time with{" "}
          <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs text-primary">wipe doctor</code>.
        </p>
      </Prose>
    </section>
  );
}

function Quickstart() {
  return (
    <section className="space-y-6">
      <H2>Quickstart</H2>
      <Prose>
        <p>Initialize a board, then create and move tickets as work progresses.</p>
      </Prose>
      <div className="space-y-5">
        <div className="space-y-2">
          <H3>1. Initialize a board</H3>
          <CodeBlock code="wipe init ." prompt />
        </div>
        <div className="space-y-2">
          <H3>2. Create a ticket</H3>
          <CodeBlock
            code={`wipe ticket create --title "Write onboarding docs" --type feature --priority high`}
            prompt
          />
        </div>
        <div className="space-y-2">
          <H3>3. Move it as work progresses</H3>
          <CodeBlock code="wipe ticket move T-1 --to in-progress" prompt />
        </div>
        <div className="space-y-2">
          <H3>4. Leave a comment</H3>
          <CodeBlock
            code={`wipe comment add T-1 --body "Blocked on the API design ticket."`}
            prompt
          />
        </div>
        <div className="space-y-2">
          <H3>5. Check the board and open the UI</H3>
          <CodeBlock code={`wipe status\nwipe serve`} prompt />
        </div>
      </div>
    </section>
  );
}

function Concepts() {
  return (
    <section className="space-y-6">
      <H2>Core concepts</H2>
      <Prose>
        <p>
          <strong className="text-foreground">Board = project.</strong> The{" "}
          <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs text-primary">.wipe/</code>{" "}
          directory <em>is</em> the board, and there is one board per project,
          created with <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs text-primary">wipe init .</code>.
        </p>
        <p>
          <strong className="text-foreground">Lists</strong> are ordered columns
          (Backlog, Todo, In Progress, Done…). Their order and each list's card
          order live in <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs text-primary">board.json</code>{" "}
          as references to ticket ids.
        </p>
        <p>
          <strong className="text-foreground">Tickets</strong> are the unit of
          work. Each ticket is its own JSON file under{" "}
          <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs text-primary">tickets/</code>,
          holding its fields (type, priority, labels, tags) and its inline
          comments. Media and attachments are version-controlled under{" "}
          <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs text-primary">media/</code>.
        </p>
      </Prose>
      <div className="space-y-2">
        <H3>The .wipe/ layout</H3>
        <CodeBlock code={WIPE_TREE} language="text" />
      </div>
      <Prose>
        <p>
          Everything except the gitignored{" "}
          <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs text-primary">.cache/index.db</code>{" "}
          is flat JSON, formatted deterministically so diffs stay readable and
          merges stay low-conflict.
        </p>
      </Prose>
    </section>
  );
}

function CliReference() {
  return (
    <section className="space-y-6">
      <H2>CLI reference</H2>
      <Prose>
        <p>
          The CLI is self-documenting — run{" "}
          <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs text-primary">wipe &lt;command&gt; --help</code>{" "}
          for any command. Add{" "}
          <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs text-primary">--json</code>{" "}
          for machine-readable output.
        </p>
      </Prose>
      <div className="space-y-10">
        {CLI_GROUPS.map((group) => (
          <div key={group.name} className="space-y-4">
            <div>
              <H3>
                <span className="font-mono text-primary">{group.name}</span>
              </H3>
              <p className="mt-1 text-sm text-muted-foreground">{group.summary}</p>
            </div>
            <div className="grid gap-4">
              {group.commands.map((c) => (
                <CommandCard
                  key={c.command}
                  command={c.command}
                  description={c.description}
                  example={c.example}
                />
              ))}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function ForAgents() {
  return (
    <section className="space-y-6">
      <H2>For agents</H2>
      <Prose>
        <p>
          wipe is designed to be driven by agents through the CLI alone — no SDK,
          no plugin. Any harness that can shell out can use it.
        </p>
      </Prose>
      <div className="space-y-2">
        <H3>The --json contract</H3>
        <Prose>
          <p>
            Every command accepts{" "}
            <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs text-primary">--json</code>{" "}
            and emits stable, machine-readable output on stdout. Parse that
            instead of scraping human text.
          </p>
        </Prose>
        <CodeBlock
          code={`wipe ticket show T-1 --json\nwipe ticket list --list todo --json\nwipe status --json`}
          prompt
        />
      </div>
      <div className="space-y-2">
        <H3>Exit codes</H3>
        <Prose>
          <p>
            Commands return stable exit codes: use them for control flow.
          </p>
        </Prose>
        <div className="overflow-hidden rounded-lg border border-border">
          <table className="w-full text-sm">
            <thead className="bg-card/60 text-left text-muted-foreground">
              <tr>
                <th className="px-4 py-2 font-medium">Code</th>
                <th className="px-4 py-2 font-medium">Meaning</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border">
              <tr>
                <td className="px-4 py-2 font-mono text-primary">0</td>
                <td className="px-4 py-2 text-muted-foreground">Success</td>
              </tr>
              <tr>
                <td className="px-4 py-2 font-mono text-primary">1</td>
                <td className="px-4 py-2 text-muted-foreground">
                  General / runtime error
                </td>
              </tr>
              <tr>
                <td className="px-4 py-2 font-mono text-primary">2</td>
                <td className="px-4 py-2 text-muted-foreground">
                  Usage error (bad arguments)
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
      <div className="space-y-2">
        <H3>The agent guide: wipe skill</H3>
        <Prose>
          <p>
            Point an agent at{" "}
            <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs text-primary">wipe skill</code>
            : it prints a self-contained guide teaching the agent how to drive the
            board — the commands, the JSON shapes, and the conventions.
          </p>
        </Prose>
        <CodeBlock code="wipe skill" prompt />
      </div>
    </section>
  );
}

const SECTION_COMPONENTS: Record<string, () => JSX.Element> = {
  introduction: Introduction,
  installation: Installation,
  quickstart: Quickstart,
  concepts: Concepts,
  cli: CliReference,
  agents: ForAgents,
};

export default function Docs() {
  const { section } = useParams();
  const current = section && SECTION_COMPONENTS[section] ? section : "introduction";
  const Section = SECTION_COMPONENTS[current];
  const idx = DOC_SECTIONS.findIndex((s) => s.slug === current);
  const next = DOC_SECTIONS[idx + 1];

  return (
    <div className="container grid gap-10 py-10 lg:grid-cols-[220px_1fr] lg:gap-14">
      {/* Sidebar */}
      <aside className="lg:sticky lg:top-24 lg:h-[calc(100vh-8rem)]">
        <nav className="flex flex-col gap-1">
          <p className="mb-2 px-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            Documentation
          </p>
          {DOC_SECTIONS.map((s) => (
            <Link
              key={s.slug}
              to={`/docs/${s.slug}`}
              className={cn(
                "rounded-md px-3 py-2 text-sm transition-colors",
                current === s.slug
                  ? "bg-secondary font-medium text-foreground"
                  : "text-muted-foreground hover:bg-secondary/50 hover:text-foreground"
              )}
            >
              {s.title}
            </Link>
          ))}
        </nav>
      </aside>

      {/* Content */}
      <div className="min-w-0 max-w-3xl">
        <Section />
        {next && (
          <div className="mt-16 flex justify-end border-t border-border/60 pt-6">
            <Link to={`/docs/${next.slug}`}>
              <Button variant="outline" className="gap-2">
                {next.title} <ArrowRight className="h-4 w-4" />
              </Button>
            </Link>
          </div>
        )}
      </div>
    </div>
  );
}
