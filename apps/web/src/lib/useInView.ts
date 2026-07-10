import { useEffect, useRef, useState } from "react";

/**
 * Reveal-on-scroll hook. Returns a ref to attach and whether the element has
 * entered the viewport yet (latched: it never flips back to false).
 *
 * Respects `prefers-reduced-motion`: when the user asks for less motion we
 * report `inView` immediately so content is shown without any transition.
 */
export function useInView<T extends HTMLElement = HTMLDivElement>(
  options: IntersectionObserverInit = {
    threshold: 0.12,
    rootMargin: "0px 0px -8% 0px",
  }
) {
  const ref = useRef<T | null>(null);
  const [inView, setInView] = useState(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    // No IntersectionObserver, or reduced motion → show immediately.
    const reduced =
      typeof window !== "undefined" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduced || typeof IntersectionObserver === "undefined") {
      setInView(true);
      return;
    }

    const obs = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          setInView(true);
          obs.disconnect();
        }
      }
    }, options);

    obs.observe(el);
    return () => obs.disconnect();
    // Options are a stable literal per call site; deliberately run once.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { ref, inView };
}
