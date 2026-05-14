import { Router } from "./app/router";
import { Toaster } from "./components/ui/Toaster";
import { ErrorBoundary } from "./lib/error_boundary";

export default function App() {
  return (
    <ErrorBoundary>
      <Toaster />
      <Router />
    </ErrorBoundary>
  );
}
