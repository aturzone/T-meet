import { Component, type ErrorInfo, type ReactNode } from "react";
import { ApiError } from "./api";

interface Props {
  children: ReactNode;
}

interface State {
  error?: Error;
}

export class ErrorBoundary extends Component<Props, State> {
  override state: State = {};

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    // Browser console only — server-side reporting is out of scope.
    console.error("error boundary caught", error, info);
  }

  override render(): ReactNode {
    const { error } = this.state;
    if (!error) return this.props.children;

    const requestId =
      error instanceof ApiError ? error.requestId : null;

    return (
      <div className="min-h-screen flex items-center justify-center bg-bg text-fg p-6">
        <div className="max-w-md text-center space-y-3">
          <h1 className="text-xl font-semibold">Something went wrong.</h1>
          <p className="text-sm text-muted">
            Please reload the page. If the problem persists, share the
            request id below with the operator.
          </p>
          {requestId && (
            <code className="block text-xs bg-surface border border-border rounded px-3 py-2">
              {requestId}
            </code>
          )}
        </div>
      </div>
    );
  }
}
