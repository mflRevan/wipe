import { Link, useParams } from "react-router-dom";
import { ArrowRight } from "lucide-react";
import { cn } from "@/lib/utils";
import { CodeBlock } from "@/components/CodeBlock";
import { CommandCard } from "@/components/CommandCard";
import { Figure } from "@/components/Figure";
import { Button } from "@/components/ui/button";
import { Tabs } from "@/pages/docs/Tabs";
import { CLI_GROUPS, DOC_SECTIONS } from "@/pages/docs/data";

const codeInline =
  "rounded-sm bg-secondary px-1 py-0.5 font-mono text-xs text-primary";

const WIPE_TREE = `.wipe/
  board.json         # lists + card order (ticket-id refs)
  definitions.json   # label definitions + priorities
  settings.json      # daemon port + exposure
  tickets/T-1.json   # one file per ticket (fields + inline comments)
  forum/F-1.json     # one file per discussion thread
  media/             # version-controlled attachments
  .cache/index.db    # gitignored, auto-managed`;

function Prose({ children }: { children: React.ReactNode }) {
  return <div className="space-y-4 text-[15px] leading-relaxed text-muted-foreground">{children}</div>;
}

function H2({ children, id }: { children: React.ReactNode; id?: string }) {
  return (
    <h2 id={id} className="scroll-mt-24 font-display text-2xl font-semibold tracking-[-0.01em] text-foreground">
      {children}
    </h2>
  );
}

function H3({ children }: { children: React.ReactNode }) {
  return <h3 className="font-display text-lg font-semibold tracking-[-0.01em] text-foreground">{children}</h3>;
}

function Introduction() {
  return (
    <section className="space-y-6">
      <H2>Introduction</H2>
      <Prose>
        <p>
          <strong className="text-foreground">wipe</strong> is a CLI-first,
          git-native task board for collaboration between humans and AI agents -
          and agents with each other. There is no external service, no separate
          database, and no account to create: the board <em>is</em> a folder in
          your repo (<code className={codeInline}>.wipe/</code>), and every change
          to it is a change you can diff, blame, branch, and merge like any other
          file.
        </p>
      </Prose>

      <Figure
        src="/screenshots/board.png"
        alt="The wipe board - a local desktop UI with lists, colored labels, priorities, and assignee avatars"
        caption="The local desktop board (wipe serve), running on the same flat JSON your agents drive from the CLI."
      />

      <div className="space-y-4">
        <H3>The problem it solves</H3>
        <Prose>
          <p>
            Coding agents are increasingly good at <em>execution</em>, but bad at
            staying aligned with the humans (and other agents) directing them.
            Specs drift, context is lost between sessions, and "what are we
            actually working on" ends up scattered across chat logs, PR
            descriptions, and someone's head. Existing trackers - Jira, Linear,
            Trello - live <em>outside</em> your repository, behind an account and
            an API your agents can't naturally reach, with their history divorced
            from your git history.
          </p>
          <p>
            wipe gives humans and agents a shared, durable, structured place to
            negotiate and track that work - without inventing a new protocol or
            standing up a hosted service. Because the board lives in the repo, it
            travels with your code and inherits git's branching, history, and
            merge semantics for free.
          </p>
        </Prose>
      </div>

      <div className="space-y-4">
        <H3>How it fits together</H3>
        <Prose>
          <p>
            A board holds ordered <strong className="text-foreground">lists</strong>{" "}
            (Backlog, Todo, In Progress, Done…) containing{" "}
            <strong className="text-foreground">tickets</strong> - the unit of
            work, each with a title, description, priority, labels, assignees, and
            an inline comment thread. All state is stored as flat,{" "}
            <strong className="text-foreground">deterministically formatted JSON</strong>{" "}
            engineered for clean diffs and low-conflict merges: board structure
            and ticket content live in separate files, so two agents editing
            different tickets never collide, and identical logical changes produce
            identical byte-for-byte diffs.
          </p>
          <p>
            Alongside the board is a git-tracked{" "}
            <strong className="text-foreground">forum</strong> - threaded
            discussions where humans and agents record the decisions, gotchas, and
            conventions a project accumulates. Tickets track <em>what</em> needs
            doing; the forum is where the team works out <em>how</em> and{" "}
            <em>why</em>, and that reasoning compounds in the repo instead of
            evaporating into chat.
          </p>
          <p>
            Agents drive everything through the self-documenting{" "}
            <code className={codeInline}>wipe</code> CLI - every command speaks{" "}
            <code className={codeInline}>--json</code>, and any harness that can
            shell out can use it. Humans use a local desktop app (
            <code className={codeInline}>wipe serve</code>) with drag-and-drop, a
            forum, and a git-history board rewind. Both sides read and write the
            exact same files.
          </p>
        </Prose>
      </div>

      <Prose>
        <p>
          New here? Continue to{" "}
          <Link
            to="/docs/installation"
            className="font-medium text-primary underline-offset-4 hover:underline"
          >
            Installation
          </Link>{" "}
          and the{" "}
          <Link
            to="/docs/quickstart"
            className="font-medium text-primary underline-offset-4 hover:underline"
          >
            Quickstart
          </Link>
          , or jump to the{" "}
          <Link
            to="/docs/humans"
            className="font-medium text-primary underline-offset-4 hover:underline"
          >
            visual tour of the desktop app
          </Link>
          .
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
                <CodeBlock code="npm install -g @mflrevan/wipe" prompt />
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
                <CodeBlock code="cargo install wipe-cli" prompt />
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
      <div className="space-y-3">
        <H3>First run: guided setup</H3>
        <Prose>
          <p>
            The first time you run any{" "}
            <code className="rounded-sm bg-secondary px-1 py-0.5 font-mono text-xs text-primary">wipe</code>{" "}
            command after installing, it offers a one-time guided setup. You can
            also start it yourself at any point:
          </p>
        </Prose>
        <CodeBlock code="wipe onboard" prompt />
        <Prose>
          <p>
            <code className="rounded-sm bg-secondary px-1 py-0.5 font-mono text-xs text-primary">wipe onboard</code>{" "}
            records your <strong className="text-foreground">machine-wide defaults</strong> - your
            identity for attribution, the default UI port and how it's exposed
            (local, Tailscale, or a reverse proxy), whether wipe starts at login,
            the starter layout for new boards, the agent-skill convention, and UI
            theme/accent. These become the defaults for every board you create.
            It's fully skippable and never blocks scripts or agents:{" "}
            <code className="rounded-sm bg-secondary px-1 py-0.5 font-mono text-xs text-primary">--json</code>,
            piped, and non-interactive runs skip the prompt, and{" "}
            <code className="rounded-sm bg-secondary px-1 py-0.5 font-mono text-xs text-primary">WIPE_NO_ONBOARD_PROMPT</code>{" "}
            disables it entirely.
          </p>
        </Prose>
      </div>
      <Prose>
        <p>
          Verify your install and environment at any time with{" "}
          <code className="rounded-sm bg-secondary px-1 py-0.5 font-mono text-xs text-primary">wipe doctor</code>.
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
        <p>
          Set your defaults once, initialize a board, then create and move tickets
          as work progresses.
        </p>
      </Prose>
      <div className="space-y-5">
        <div className="space-y-2">
          <H3>1. Set up your defaults (once per machine)</H3>
          <Prose>
            <p>
              Offered automatically on your first run, or start it yourself. Sets
              your identity, UI port, autostart, and styling for every board.
            </p>
          </Prose>
          <CodeBlock code="wipe onboard" prompt />
        </div>
        <div className="space-y-2">
          <H3>2. Initialize a board</H3>
          <CodeBlock code="wipe init ." prompt />
        </div>
        <div className="space-y-2">
          <H3>3. Create a ticket</H3>
          <CodeBlock
            code={`wipe ticket create --title "Write onboarding docs" --priority high`}
            prompt
          />
        </div>
        <div className="space-y-2">
          <H3>4. Move it as work progresses</H3>
          <CodeBlock code="wipe ticket move T-1 --to in-progress" prompt />
        </div>
        <div className="space-y-2">
          <H3>5. Leave a comment</H3>
          <CodeBlock
            code={`wipe comment add T-1 --body "Blocked on the API design ticket."`}
            prompt
          />
        </div>
        <div className="space-y-2">
          <H3>6. Check the board and open the UI</H3>
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
          <code className="rounded-sm bg-secondary px-1 py-0.5 font-mono text-xs text-primary">.wipe/</code>{" "}
          directory <em>is</em> the board, and there is one board per project,
          created with <code className="rounded-sm bg-secondary px-1 py-0.5 font-mono text-xs text-primary">wipe init .</code>.
        </p>
        <p>
          <strong className="text-foreground">Lists</strong> are ordered columns
          (Backlog, Todo, In Progress, Done…). Their order and each list's card
          order live in <code className="rounded-sm bg-secondary px-1 py-0.5 font-mono text-xs text-primary">board.json</code>{" "}
          as references to ticket ids.
        </p>
        <p>
          <strong className="text-foreground">Tickets</strong> are the unit of
          work. Each ticket is its own JSON file under{" "}
          <code className={codeInline}>tickets/</code>, holding its fields
          (priority, labels, assignees), a checklist, acceptance criteria, and its
          inline comments. Media and attachments are version-controlled under{" "}
          <code className={codeInline}>media/</code>.
        </p>
        <p>
          <strong className="text-foreground">The forum</strong> is a parallel,
          git-tracked space for discussion that outlives any single ticket:
          threaded posts under <code className={codeInline}>forum/</code> where
          humans and agents settle decisions and record gotchas and conventions.
          Posts nest into reply trees (<code className={codeInline}>F-1</code>,{" "}
          <code className={codeInline}>F-1.2</code>), carry labels from the board's
          pool, and are fully searchable - so a project's reasoning accumulates in
          the repo. Agents can subscribe to it with{" "}
          <code className={codeInline}>wipe forum watch</code>.
        </p>
        <p>
          <strong className="text-foreground">Identity &amp; attribution.</strong>{" "}
          Every action is attributed to a human or an agent. Identities are
          resolved from your version control by default, and agents can bind their
          own with <code className={codeInline}>wipe identity use</code> or a
          per-command <code className={codeInline}>--agentid</code> - so the board,
          comments, and forum always show who (or what) did what.
        </p>
      </Prose>
      <div className="space-y-2">
        <H3>The .wipe/ layout</H3>
        <CodeBlock code={WIPE_TREE} language="text" />
      </div>
      <Prose>
        <p>
          Everything except the gitignored{" "}
          <code className="rounded-sm bg-secondary px-1 py-0.5 font-mono text-xs text-primary">.cache/index.db</code>{" "}
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
          The CLI is self-documenting - run{" "}
          <code className="rounded-sm bg-secondary px-1 py-0.5 font-mono text-xs text-primary">wipe &lt;command&gt; --help</code>{" "}
          for any command. Add{" "}
          <code className="rounded-sm bg-secondary px-1 py-0.5 font-mono text-xs text-primary">--json</code>{" "}
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
          wipe is designed to be driven by agents through the CLI alone - no SDK,
          no plugin. Any harness that can shell out can use it.
        </p>
      </Prose>
      <div className="space-y-2">
        <H3>The --json contract</H3>
        <Prose>
          <p>
            Every command accepts{" "}
            <code className="rounded-sm bg-secondary px-1 py-0.5 font-mono text-xs text-primary">--json</code>{" "}
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
        <div className="overflow-hidden rounded-md border border-border">
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
            <code className="rounded-sm bg-secondary px-1 py-0.5 font-mono text-xs text-primary">wipe skill</code>
            : it prints a self-contained guide teaching the agent how to drive the
            board - the commands, the JSON shapes, and the conventions.
          </p>
        </Prose>
        <CodeBlock code="wipe skill" prompt />
      </div>
    </section>
  );
}

function ForHumans() {
  return (
    <section className="space-y-6">
      <H2>For humans</H2>
      <Prose>
        <p>
          You don't drive wipe from the CLI - you get a real desktop app. Run{" "}
          <code className={codeInline}>wipe serve</code> in any project (or
          anywhere, to browse every board you've opened) and it launches a local
          board at <code className={codeInline}>localhost</code>, served by a
          lightweight daemon. It's the same flat JSON your agents write, rendered
          as something you can actually work in - and it updates live as agents
          make changes.
        </p>
      </Prose>

      <div className="space-y-3">
        <H3>The board</H3>
        <Prose>
          <p>
            Lists as columns, tickets as cards. Drag cards between lists to move
            work along; colored labels, priority dots, comment counts, and
            assignee avatars are all visible at a glance. Use the header to switch
            projects, jump to the forum, or open history. When an agent changes the
            board, the card animates and briefly highlights - so you can watch work
            happen.
          </p>
        </Prose>
        <Figure
          src="/screenshots/board.png"
          alt="The wipe board with lists, colored labels, priorities, and assignee avatars"
        />
      </div>

      <div className="space-y-3">
        <H3>Opening a ticket</H3>
        <Prose>
          <p>
            Click a card for a focused editor: title and description, labels,
            assignees (human or agent), priority, a{" "}
            <strong className="text-foreground">checklist</strong>,{" "}
            <strong className="text-foreground">acceptance criteria</strong>, and
            attachments on the left; an{" "}
            <strong className="text-foreground">activity feed</strong> on the right.
            That feed is the human ↔ agent channel - below, an agent (
            <code className={codeInline}>planner-bot</code>) created the card and
            flagged a blocker, and a human replied with the decision. Break work
            into checklist items and either side can tick them off (
            <code className={codeInline}>wipe checklist</code> from the CLI). The{" "}
            <strong className="text-foreground">acceptance criteria</strong> are the
            reviewer's list - the conditions a ticket must meet to be accepted (
            <code className={codeInline}>wipe criteria</code>): a reviewer ticks each
            one, or bounces the ticket back with the unmet ones visible, so the
            worker sees exactly what's left. Both show a{" "}
            <code className={codeInline}>2/3</code> badge right on the card.
          </p>
        </Prose>
        <Figure
          src="/screenshots/ticket.png"
          alt="A wipe ticket open, showing labels, members, priority, and an activity feed with an agent and a human"
        />
      </div>

      <div className="space-y-3">
        <H3>The forum</H3>
        <Prose>
          <p>
            The <strong className="text-foreground">Forum</strong> tab is where
            longer-lived discussion lives. Threads nest into reply trees, carry
            labels, and render light Markdown - a durable, searchable record of the
            decisions and gotchas behind the work, tracked in git right next to it.
            Everything here is also reachable from the CLI via{" "}
            <code className={codeInline}>wipe forum</code>, so agents take part in
            the same conversations.
          </p>
        </Prose>
        <Figure
          src="/screenshots/forum.png"
          alt="The wipe forum with a threaded discussion between a human and an agent"
        />
      </div>

      <div className="space-y-3">
        <H3>Rewind the history</H3>
        <Prose>
          <p>
            Because every board change is a git commit, the{" "}
            <strong className="text-foreground">History</strong> view is a timeline
            of your actual commits, with board checkpoints marked. Jump to any past
            state to see exactly how the board looked then, and who - human or agent
            - moved it there. No separate audit log; it's just your git history,
            made legible.
          </p>
        </Prose>
        <Figure
          src="/screenshots/history.png"
          alt="The wipe commit history over the board, with board checkpoints and per-commit attribution"
        />
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
  humans: ForHumans,
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
