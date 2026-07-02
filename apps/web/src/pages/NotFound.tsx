import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button";

export default function NotFound() {
  return (
    <div className="container flex flex-col items-center justify-center gap-4 py-32 text-center">
      <p className="font-mono text-sm text-primary">404</p>
      <h1 className="text-3xl font-bold tracking-tight">Page not found</h1>
      <p className="max-w-md text-muted-foreground">
        That page moved to a different list on the board.
      </p>
      <Link to="/">
        <Button>Back home</Button>
      </Link>
    </div>
  );
}
