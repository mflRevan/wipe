import { useState } from "react";
import { Check, Copy, Terminal } from "lucide-react";
import { cn } from "@/lib/utils";

interface CodeBlockProps {
  code: string;
  /** Optional label shown in the title bar, e.g. "bash". */
  language?: string;
  /** Show a leading `$` prompt on each line (shell style). */
  prompt?: boolean;
  className?: string;
}

export function CodeBlock({
  code,
  language = "bash",
  prompt = false,
  className,
}: CodeBlockProps) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      /* clipboard unavailable */
    }
  };

  const lines = code.split("\n");

  return (
    <div
      className={cn(
        "group relative overflow-hidden rounded-md border border-border bg-card",
        className
      )}
    >
      <div className="flex items-center justify-between border-b border-border/70 px-4 py-2">
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <Terminal className="h-3.5 w-3.5" />
          <span className="font-mono">{language}</span>
        </div>
        <button
          onClick={copy}
          aria-label="Copy code"
          className="flex items-center gap-1.5 rounded-sm px-2 py-1 text-xs text-muted-foreground transition-colors duration-wp-fast hover:bg-elevated hover:text-foreground"
        >
          {copied ? (
            <Check className="h-3.5 w-3.5 text-primary" />
          ) : (
            <Copy className="h-3.5 w-3.5" />
          )}
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <pre className="overflow-x-auto px-4 py-3.5 text-sm leading-relaxed">
        <code className="font-mono">
          {lines.map((line, i) => (
            <span key={i} className="block">
              {prompt && line.trim() !== "" && (
                <span className="select-none text-primary/70">$ </span>
              )}
              {line || " "}
            </span>
          ))}
        </code>
      </pre>
    </div>
  );
}
