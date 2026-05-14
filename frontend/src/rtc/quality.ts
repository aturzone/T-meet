/** Quality classifier driven by RTCP stats. */

export type QualityLevel = "good" | "ok" | "bad";

export interface QualityInput {
  /** 0..1 fraction of packets lost in the most recent stats window. */
  packetLossFraction: number;
  /** Jitter in ms. */
  jitterMs: number;
}

export function classifyQuality(input: QualityInput): QualityLevel {
  if (input.packetLossFraction > 0.05) return "bad";
  if (input.jitterMs > 50) return "ok";
  if (input.packetLossFraction > 0.02) return "ok";
  return "good";
}
