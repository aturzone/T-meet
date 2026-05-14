/** Active-speaker detection. Tracks per-peer audio levels over a sliding
 *  window; a new "loudest" peer only takes the crown after winning two
 *  consecutive windows so we don't flicker on brief noise spikes. */

export interface ActiveSpeakerSample {
  pid: string;
  level: number;
}

export class ActiveSpeakerTracker {
  private latest = new Map<string, number>();
  private lastWinner: string | null = null;
  private prevTopCandidate: string | null = null;

  /** Feed the current window's samples. Returns the *active* peer (after the
   *  two-window confirmation), or null. */
  push(samples: ActiveSpeakerSample[]): string | null {
    this.latest.clear();
    for (const s of samples) {
      this.latest.set(s.pid, s.level);
    }

    let topPid: string | null = null;
    let topLevel = 0;
    for (const [pid, level] of this.latest) {
      if (level > topLevel) {
        topLevel = level;
        topPid = pid;
      }
    }

    if (topPid === null) {
      this.prevTopCandidate = null;
      return this.lastWinner;
    }
    if (topPid === this.prevTopCandidate && topPid !== this.lastWinner) {
      this.lastWinner = topPid;
    } else if (topPid === this.lastWinner) {
      // Already winning — stay.
    }
    this.prevTopCandidate = topPid;
    return this.lastWinner;
  }

  reset(): void {
    this.latest.clear();
    this.lastWinner = null;
    this.prevTopCandidate = null;
  }
}
