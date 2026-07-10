import { cn } from "@/lib/utils";

type FigureProps = {
  src: string;
  alt: string;
  /** Optional caption rendered under the frame. */
  caption?: React.ReactNode;
  className?: string;
  /** Subtle hover lift (for standalone showcase shots). */
  interactive?: boolean;
};

/**
 * A framed product screenshot: rounded card with a border and shadow so a dark
 * app capture reads cleanly against either theme, with an optional caption.
 */
export function Figure({ src, alt, caption, className, interactive = false }: FigureProps) {
  return (
    <figure className={cn("space-y-3", className)}>
      <div
        className={cn(
          "overflow-hidden rounded-lg border border-border bg-card shadow-card",
          interactive &&
            "transition-shadow duration-wp-base ease-wp-ease hover:shadow-lift"
        )}
      >
        <img src={src} alt={alt} loading="lazy" className="block w-full" />
      </div>
      {caption && (
        <figcaption className="text-sm leading-relaxed text-muted-foreground">
          {caption}
        </figcaption>
      )}
    </figure>
  );
}
