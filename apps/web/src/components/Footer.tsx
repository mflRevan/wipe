import { Link } from "react-router-dom";
import { Logo } from "@/components/Logo";
import { REPO_URL } from "@/lib/constants";

export function Footer() {
  return (
    <footer className="border-t border-border/60">
      <div className="container flex flex-col items-start justify-between gap-6 py-10 sm:flex-row sm:items-center">
        <div className="space-y-2">
          <Logo />
          <p className="max-w-sm text-sm text-muted-foreground">
            A git-native task board for humans and agents. Pre-1.0, under active
            development.
          </p>
        </div>
        <div className="flex flex-col gap-2 text-sm text-muted-foreground sm:items-end">
          <Link to="/docs" className="hover:text-foreground">
            Documentation
          </Link>
          <a href={REPO_URL} target="_blank" rel="noreferrer" className="hover:text-foreground">
            GitHub
          </a>
          <span className="text-xs">MIT</span>
        </div>
      </div>
    </footer>
  );
}
