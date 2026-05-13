"use client";

import { Component, type ReactNode } from "react";

import { Button } from "./Button";
import { Skeleton } from "./Skeleton";

// ── Error boundary ────────────────────────────────────────────────────────────

interface ErrorBoundaryProps {
  children: ReactNode;
  fallback?: (error: Error, reset: () => void) => ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  override state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: unknown): ErrorBoundaryState {
    return { error: error instanceof Error ? error : new Error(String(error)) };
  }

  reset = () => this.setState({ error: null });

  override render() {
    const { error } = this.state;
    if (error) {
      if (this.props.fallback) return this.props.fallback(error, this.reset);
      return (
        <div className="flex flex-col items-center gap-3 py-8 text-center">
          <p className="text-sm text-rose-400">{error.message}</p>
          <Button variant="secondary" size="sm" onClick={this.reset}>
            Retry
          </Button>
        </div>
      );
    }
    return this.props.children;
  }
}

// ── AsyncBoundary ─────────────────────────────────────────────────────────────

interface AsyncBoundaryProps {
  isLoading?: boolean;
  isEmpty?: boolean;
  /** Rendered while loading. Defaults to a generic skeleton. */
  loadingFallback?: ReactNode;
  /** Rendered when the data set is empty. */
  emptyFallback?: ReactNode;
  children: ReactNode;
}

/**
 * PRD §17.7 — every async surface has all four states: loading, empty, error,
 * and success. Wrap any data-driven section with this component.
 */
export function AsyncBoundary({
  isLoading,
  isEmpty,
  loadingFallback,
  emptyFallback,
  children,
}: AsyncBoundaryProps) {
  if (isLoading) {
    return (
      loadingFallback ?? (
        <div className="flex flex-col gap-2 p-4" aria-busy role="status" aria-label="Loading">
          <Skeleton className="h-4 w-3/4" />
          <Skeleton className="h-4 w-1/2" />
          <Skeleton className="h-4 w-5/6" />
        </div>
      )
    );
  }

  if (isEmpty) {
    return (
      emptyFallback ?? (
        <div className="flex flex-col items-center gap-2 py-8 text-center" role="status">
          <p className="text-sm text-zinc-400">Nothing here yet.</p>
        </div>
      )
    );
  }

  return <ErrorBoundary>{children}</ErrorBoundary>;
}
