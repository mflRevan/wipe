import { cn } from "@/lib/utils";
import { useInView } from "@/lib/useInView";

type RevealProps = {
  children: React.ReactNode;
  className?: string;
  /** Delay in ms before this element eases in (for staggering siblings). */
  delay?: number;
  as?: keyof JSX.IntrinsicElements;
};

/**
 * Fades and lifts its children in as they scroll into view. Subtle by design
 * (8px rise, one-shot), and a no-op under `prefers-reduced-motion` (the hook
 * reports visible immediately and the global motion reset zeroes the duration).
 */
export function Reveal({ children, className, delay = 0, as = "div" }: RevealProps) {
  const { ref, inView } = useInView<HTMLElement>();
  const Tag = as as React.ElementType;
  return (
    <Tag
      ref={ref}
      style={{ transitionDelay: inView ? `${delay}ms` : "0ms" }}
      className={cn(
        "transition-[opacity,transform] duration-500 ease-wp-ease will-change-transform",
        inView ? "translate-y-0 opacity-100" : "translate-y-2 opacity-0",
        className
      )}
    >
      {children}
    </Tag>
  );
}
