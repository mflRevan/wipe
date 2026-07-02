import { Link, useLocation } from "react-router-dom";
import { Github } from "lucide-react";
import { Logo } from "@/components/Logo";
import { Button } from "@/components/ui/button";
import { REPO_URL } from "@/lib/constants";
import { cn } from "@/lib/utils";

export function Navbar() {
  const { pathname } = useLocation();

  const navItem = (to: string, label: string) => (
    <Link
      to={to}
      className={cn(
        "text-sm transition-colors hover:text-foreground",
        pathname === to || (to === "/docs" && pathname.startsWith("/docs"))
          ? "text-foreground"
          : "text-muted-foreground"
      )}
    >
      {label}
    </Link>
  );

  return (
    <header className="sticky top-0 z-50 border-b border-border/60 bg-background/80 backdrop-blur-md">
      <div className="container flex h-16 items-center justify-between">
        <div className="flex items-center gap-8">
          <Link to="/" aria-label="wipe home">
            <Logo />
          </Link>
          <nav className="hidden items-center gap-6 md:flex">
            {navItem("/", "Home")}
            {navItem("/docs", "Docs")}
          </nav>
        </div>
        <div className="flex items-center gap-2">
          <a href={REPO_URL} target="_blank" rel="noreferrer">
            <Button variant="ghost" size="sm" className="gap-2">
              <Github className="h-4 w-4" />
              <span className="hidden sm:inline">GitHub</span>
            </Button>
          </a>
          <Link to="/docs" className="hidden sm:block">
            <Button size="sm">Get started</Button>
          </Link>
        </div>
      </div>
    </header>
  );
}
