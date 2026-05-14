import { Link } from "react-router-dom";
import { Card } from "../components/ui/Card";

export default function NotFound() {
  return (
    <main className="min-h-screen flex items-center justify-center p-6">
      <Card className="max-w-md text-center space-y-3">
        <h1 className="text-xl font-semibold">Couldn't find that page.</h1>
        <p className="text-sm text-muted">
          Check the room link from your host, or go back to the landing
          page.
        </p>
        <Link to="/" className="text-accent underline text-sm">
          ← Back home
        </Link>
      </Card>
    </main>
  );
}
