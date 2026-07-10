import { Link } from "react-router-dom";
import {
  ArrowRight,
  GitBranch,
  Github,
  Terminal,
  Bot,
  MousePointerClick,
  History,
  Users,
  FileJson,
  Boxes,
  MessagesSquare,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { CodeBlock } from "@/components/CodeBlock";
import { Figure } from "@/components/Figure";
import { Reveal } from "@/components/Reveal";
import { REPO_URL, TAGLINE } from "@/lib/constants";

const whyItems = [
  {
    icon: GitBranch,
    title: "Git-native by design",
    body: "The board is a folder in your repo (.wipe/). Every change is something you can diff, blame, branch, and merge like any other file - no service, no database, no account.",
  },
  {
    icon: Boxes,
    title: "Works with any agent harness",
    body: "Agents talk to the board exclusively through the wipe CLI. If a harness can shell out, it can drive the board - no SDK, plugin, or proprietary integration required.",
  },
  {
    icon: Users,
    title: "Humans and agents share one board",
    body: "The same flat JSON powers a CLI for agents and a local desktop app for humans. Everyone reads and writes the same durable, structured source of truth.",
  },
  {
    icon: FileJson,
    title: "Spec-driven coordination",
    body: "Tickets and a git-tracked forum give humans and agents a shared place to negotiate scope and record decisions, so specs stop drifting across chat logs and PR descriptions.",
  },
];

const features = [
  {
    icon: Terminal,
    title: "CLI-first & self-documenting",
    body: "Every command has --help and stable exit codes. Agents discover the whole surface without a manual.",
  },
  {
    icon: GitBranch,
    title: "Git-native flat JSON",
    body: "Deterministically formatted JSON under .wipe/, engineered for clean diffs and low-conflict merges.",
  },
  {
    icon: Bot,
    title: "Human ↔ agent & agent ↔ agent",
    body: "A shared board for people and models - and for agents coordinating directly with each other.",
  },
  {
    icon: MousePointerClick,
    title: "Local drag-and-drop UI",
    body: "wipe serve launches a local desktop app with a real board you can drag cards around on.",
  },
  {
    icon: History,
    title: "Git-history board rewind",
    body: "Scrub through past board states with GitLens-style attribution for who changed what, and when.",
  },
  {
    icon: MessagesSquare,
    title: "A git-tracked forum",
    body: "Threaded discussions where humans and agents record decisions and gotchas - knowledge that compounds in the repo.",
  },
];

const showcase = [
  {
    img: "/screenshots/ticket.png",
    alt: "A wipe ticket open, with labels, assignees, priority, and an activity feed",
    title: "Tickets built for a shared workflow",
    body: "Labels, assignees, priority, description, and attachments - plus an activity feed that is the human ↔ agent channel. Here an agent flags a blocker and a human makes the call.",
  },
  {
    img: "/screenshots/forum.png",
    alt: "The wipe forum with a threaded discussion between a human and an agent",
    title: "A forum where decisions compound",
    body: "Beyond tickets, threaded discussions capture the decisions, gotchas, and conventions a project accumulates. It's git-tracked, searchable, and agents can subscribe to it with wipe forum watch.",
  },
  {
    img: "/screenshots/history.png",
    alt: "The wipe commit history with board checkpoints",
    title: "Rewind the board through git",
    body: "Every board change is a commit. Scrub the timeline, see who - human or agent - changed what, and jump to any past state. No separate audit log; it's just your git history.",
  },
];

export default function Landing() {
  return (
    <>
      {/* Hero */}
      <section className="relative overflow-hidden">
        {/* Soft accent glow behind the hero. */}
        <div
          aria-hidden
          className="pointer-events-none absolute left-1/2 top-[-10%] h-[420px] w-[820px] -translate-x-1/2 rounded-pill bg-primary/10 blur-[120px]"
        />
        <div className="container relative flex flex-col items-center py-24 text-center md:py-28">
          <a
            href={REPO_URL}
            target="_blank"
            rel="noreferrer"
            className="animate-fade-up mb-6 inline-flex items-center gap-2 rounded-full border border-border bg-card px-3 py-1 text-xs text-muted-foreground transition-colors duration-wp-fast hover:text-foreground"
          >
            <span className="h-1.5 w-1.5 rounded-full bg-primary" />
            Pre-1.0 · open source · MIT
          </a>
          <h1 className="animate-fade-up max-w-3xl font-display text-4xl font-semibold tracking-[-0.01em] [animation-delay:60ms] sm:text-6xl">
            A git-native task board
            <br />
            for humans and agents.
          </h1>
          <p className="animate-fade-up mt-6 max-w-2xl text-lg text-muted-foreground [animation-delay:140ms]">
            wipe is a CLI-first task board that lives inside your git repository.
            Agents drive it through a self-documenting CLI; humans get a local
            drag-and-drop desktop app - over one shared, diffable source of truth.
          </p>
          <div className="animate-fade-up mt-8 flex flex-col gap-3 [animation-delay:220ms] sm:flex-row">
            <Link to="/docs">
              <Button size="lg" className="gap-2">
                Get started <ArrowRight className="h-4 w-4" />
              </Button>
            </Link>
            <a href={REPO_URL} target="_blank" rel="noreferrer">
              <Button size="lg" variant="outline" className="gap-2">
                <Github className="h-4 w-4" /> GitHub
              </Button>
            </a>
          </div>
          <p className="animate-fade-up mt-4 font-mono text-xs text-muted-foreground [animation-delay:280ms]">
            {TAGLINE}
          </p>

          <div className="animate-fade-up mt-10 w-full max-w-lg text-left [animation-delay:340ms]">
            <CodeBlock
              code={`npm install -g @mflrevan/wipe\nwipe onboard   # one-time guided setup\nwipe init .`}
              prompt
            />
          </div>

          {/* Hero product shot. */}
          <div className="animate-fade-up mt-14 w-full max-w-5xl [animation-delay:420ms]">
            <Figure
              src="/screenshots/board.png"
              alt="The wipe board - a local drag-and-drop desktop UI with lists, colored labels, and assignees"
            />
          </div>
        </div>
      </section>

      {/* Why wipe */}
      <section className="border-t border-border/60 py-20">
        <div className="container">
          <Reveal className="mx-auto max-w-2xl text-center">
            <h2 className="font-display text-3xl font-semibold tracking-[-0.01em]">Why wipe</h2>
            <p className="mt-3 text-muted-foreground">
              Coding agents are good at execution but bad at staying aligned. wipe
              gives humans and agents a shared, durable place to track the work.
            </p>
          </Reveal>
          <div className="mt-12 grid gap-6 md:grid-cols-2">
            {whyItems.map((item, i) => (
              <Reveal
                key={item.title}
                delay={i * 60}
                className="group rounded-md border border-border bg-card p-6 transition-all duration-wp-base ease-wp-ease hover:-translate-y-0.5 hover:border-border-strong hover:shadow-card"
              >
                <div className="mb-4 grid h-10 w-10 place-items-center rounded-md bg-primary/10 text-primary transition-colors duration-wp-base group-hover:bg-primary/15">
                  <item.icon className="h-5 w-5" />
                </div>
                <h3 className="font-display text-lg font-semibold tracking-[-0.01em]">{item.title}</h3>
                <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
                  {item.body}
                </p>
              </Reveal>
            ))}
          </div>
        </div>
      </section>

      {/* Features grid */}
      <section className="border-t border-border/60 py-20">
        <div className="container">
          <Reveal className="mx-auto max-w-2xl text-center">
            <h2 className="font-display text-3xl font-semibold tracking-[-0.01em]">
              Built for the way agents work
            </h2>
            <p className="mt-3 text-muted-foreground">
              Everything is a file. Everything is scriptable. Everything merges.
            </p>
          </Reveal>
          <div className="mt-12 grid gap-px overflow-hidden rounded-md border border-border bg-border sm:grid-cols-2 lg:grid-cols-3">
            {features.map((f, i) => (
              <Reveal
                key={f.title}
                delay={(i % 3) * 60}
                className="group bg-background p-6 transition-colors duration-wp-base hover:bg-card"
              >
                <f.icon className="h-5 w-5 text-primary transition-transform duration-wp-base group-hover:scale-110" />
                <h3 className="mt-4 font-display font-semibold tracking-[-0.01em]">{f.title}</h3>
                <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
                  {f.body}
                </p>
              </Reveal>
            ))}
          </div>
        </div>
      </section>

      {/* Visual showcase */}
      <section className="border-t border-border/60 py-20">
        <div className="container">
          <Reveal className="mx-auto max-w-2xl text-center">
            <h2 className="font-display text-3xl font-semibold tracking-[-0.01em]">
              One board. One repo. Every angle.
            </h2>
            <p className="mt-3 text-muted-foreground">
              The local desktop app, running on the same flat JSON your agents drive
              from the CLI.
            </p>
          </Reveal>
          <div className="mt-14 space-y-16">
            {showcase.map((s, i) => (
              <Reveal
                key={s.title}
                className="grid items-center gap-8 lg:grid-cols-2 lg:gap-12"
              >
                <div className={i % 2 === 1 ? "lg:order-2" : ""}>
                  <Figure src={s.img} alt={s.alt} interactive />
                </div>
                <div className={i % 2 === 1 ? "lg:order-1" : ""}>
                  <h3 className="font-display text-2xl font-semibold tracking-[-0.01em]">
                    {s.title}
                  </h3>
                  <p className="mt-4 leading-relaxed text-muted-foreground">{s.body}</p>
                </div>
              </Reveal>
            ))}
          </div>
        </div>
      </section>

      {/* Quickstart */}
      <section className="border-t border-border/60 py-20">
        <div className="container grid gap-10 lg:grid-cols-2 lg:items-center">
          <Reveal>
            <h2 className="font-display text-3xl font-semibold tracking-[-0.01em]">
              From zero to a shared board in seconds
            </h2>
            <p className="mt-4 text-muted-foreground">
              Set your defaults once, initialize a board, then create and move
              tickets - all from the CLI, all committed to git. Add{" "}
              <code className="rounded-sm bg-secondary px-1.5 py-0.5 font-mono text-xs text-primary">
                --json
              </code>{" "}
              to any command for machine-readable output.
            </p>
            <div className="mt-6">
              <Link to="/docs">
                <Button variant="outline" className="gap-2">
                  Read the docs <ArrowRight className="h-4 w-4" />
                </Button>
              </Link>
            </div>
          </Reveal>
          <Reveal delay={80}>
            <CodeBlock code={quickstart} prompt />
          </Reveal>
        </div>
      </section>
    </>
  );
}

const quickstart = `# One-time guided setup (offered automatically on your first run)
wipe onboard

# Initialize a board in your project
wipe init .

# Create a ticket
wipe ticket create --title "Write onboarding docs" --priority high

# Move it as work progresses
wipe ticket move T-1 --to in-progress

# Leave a comment for whoever picks it up next
wipe comment add T-1 --body "Blocked on the API design ticket."

# Talk out a decision in the forum
wipe forum post --title "Token store: encrypt at rest?" --label backend

# Check the board and launch the local UI
wipe status
wipe serve`;
