/** Exponential backoff scheduler: 1s, 2s, 4s, 8s, 16s, capped at 30s. */

export interface BackoffOptions {
  baseMs?: number;
  capMs?: number;
}

export class Backoff {
  private attempts = 0;
  private readonly baseMs: number;
  private readonly capMs: number;

  constructor(options: BackoffOptions = {}) {
    this.baseMs = options.baseMs ?? 1000;
    this.capMs = options.capMs ?? 30_000;
  }

  /** Returns the next delay (ms) and advances the counter. */
  next(): number {
    const delay = Math.min(this.capMs, this.baseMs * 2 ** this.attempts);
    this.attempts += 1;
    return delay;
  }

  /** Reset on success. */
  reset(): void {
    this.attempts = 0;
  }

  get attemptCount(): number {
    return this.attempts;
  }
}
