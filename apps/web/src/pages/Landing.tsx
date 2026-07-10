import { Link } from "react-router-dom";
import {
  ArrowRight,
  GitBranch,
  Github,
  Terminal,
  Bot,
  MousePointerClick,
  History,
  Braces,
  Users,
  FileJson,
  Boxes,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { CodeBlock } from "@/components/CodeBlock";
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
    body: "Tickets and comments give humans and agents a shared place to negotiate scope and track work, so specs stop drifting across chat logs and PR descriptions.",
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
    icon: Braces,
    title: "--json everywhere",
    body: "Pass --json to any command for stable, machine-readable output built for agent consumption.",
  },
];

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

# Check the board at a glance
wipe status

# Launch the local desktop UI
wipe serve`;

export default function Landing() {
  return (
    <>
      {/* Hero */}
      <section className="relative overflow-hidden">
        <div className="container relative flex flex-col items-center py-24 text-center md:py-32">
          <a
            href={REPO_URL}
            target="_blank"
            rel="noreferrer"
            className="animate-fade-up mb-6 inline-flex items-center gap-2 rounded-full border border-border bg-card px-3 py-1 text-xs text-muted-foreground transition-colors duration-wp-fast hover:text-foreground"
          >
            <span className="h-1.5 w-1.5 rounded-full bg-primary" />
            Pre-1.0 · open source · MIT
          </a>
          <h1 className="animate-fade-up max-w-3xl font-display text-4xl font-semibold tracking-[-0.01em] sm:text-6xl">
            A git-native task board
            <br />
            for humans and agents.
          </h1>
          <p className="animate-fade-up mt-6 max-w-2xl text-lg text-muted-foreground">
            wipe is a CLI-first task board that lives inside your git repository.
            Agents drive it through a self-documenting CLI; humans get a local
            drag-and-drop desktop app - over one shared, diffable source of truth.
          </p>
          <div className="animate-fade-up mt-8 flex flex-col gap-3 sm:flex-row">
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
          <p className="mt-4 font-mono text-xs text-muted-foreground">{TAGLINE}</p>

          <div className="animate-fade-up mt-14 w-full max-w-2xl text-left">
            <CodeBlock
              code={`npm install -g @mflrevan/wipe\nwipe onboard   # one-time guided setup\nwipe init .`}
              prompt
            />
          </div>
        </div>
      </section>

      {/* Why wipe */}
      <section className="border-t border-border/60 py-20">
        <div className="container">
          <div className="mx-auto max-w-2xl text-center">
            <h2 className="font-display text-3xl font-semibold tracking-[-0.01em]">Why wipe</h2>
            <p className="mt-3 text-muted-foreground">
              Coding agents are good at execution but bad at staying aligned. wipe
              gives humans and agents a shared, durable place to track the work.
            </p>
          </div>
          <div className="mt-12 grid gap-6 md:grid-cols-2">
            {whyItems.map((item) => (
              <div
                key={item.title}
                className="rounded-md border border-border bg-card p-6 transition-shadow duration-wp-fast hover:shadow-card"
              >
                <div className="mb-4 grid h-10 w-10 place-items-center rounded-md bg-primary/10 text-primary">
                  <item.icon className="h-5 w-5" />
                </div>
                <h3 className="font-display text-lg font-semibold tracking-[-0.01em]">{item.title}</h3>
                <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
                  {item.body}
                </p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Features grid */}
      <section className="border-t border-border/60 py-20">
        <div className="container">
          <div className="mx-auto max-w-2xl text-center">
            <h2 className="font-display text-3xl font-semibold tracking-[-0.01em]">
              Built for the way agents work
            </h2>
            <p className="mt-3 text-muted-foreground">
              Everything is a file. Everything is scriptable. Everything merges.
            </p>
          </div>
          <div className="mt-12 grid gap-px overflow-hidden rounded-md border border-border bg-border sm:grid-cols-2 lg:grid-cols-3">
            {features.map((f) => (
              <div key={f.title} className="bg-background p-6">
                <f.icon className="h-5 w-5 text-primary" />
                <h3 className="mt-4 font-display font-semibold tracking-[-0.01em]">{f.title}</h3>
                <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
                  {f.body}
                </p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Quickstart */}
      <section className="border-t border-border/60 py-20">
        <div className="container grid gap-10 lg:grid-cols-2 lg:items-center">
          <div>
            <h2 className="font-display text-3xl font-semibold tracking-[-0.01em]">
              From zero to a shared board in seconds
            </h2>
            <p className="mt-4 text-muted-foreground">
              Initialize a board, create tickets, move them across lists, and
              leave comments - all from the CLI, all committed to git. Add{" "}
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
          </div>
          <CodeBlock code={quickstart} prompt />
        </div>
      </section>
    </>
  );
}
