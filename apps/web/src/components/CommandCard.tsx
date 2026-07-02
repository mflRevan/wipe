import { CodeBlock } from "@/components/CodeBlock";

interface CommandCardProps {
  command: string;
  description: string;
  example: string;
}

export function CommandCard({ command, description, example }: CommandCardProps) {
  return (
    <div className="rounded-md border border-border bg-card p-4">
      <div className="mb-1 flex flex-wrap items-baseline gap-2">
        <code className="rounded-sm bg-secondary px-1.5 py-0.5 font-mono text-sm text-primary">
          {command}
        </code>
      </div>
      <p className="mb-3 text-sm text-muted-foreground">{description}</p>
      <CodeBlock code={example} prompt />
    </div>
  );
}
