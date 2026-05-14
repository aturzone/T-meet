import { describe, it, expect } from "vitest";
import { Backoff } from "../rtc/reconnect";
import { classifyQuality } from "../rtc/quality";
import { ActiveSpeakerTracker } from "../rtc/active_speaker";

describe("Backoff", () => {
  it("produces the documented sequence", () => {
    const b = new Backoff();
    expect(b.next()).toBe(1000);
    expect(b.next()).toBe(2000);
    expect(b.next()).toBe(4000);
    expect(b.next()).toBe(8000);
    expect(b.next()).toBe(16000);
    expect(b.next()).toBe(30000); // capped
    expect(b.next()).toBe(30000);
  });
  it("resets to 1s after success", () => {
    const b = new Backoff();
    b.next();
    b.next();
    b.reset();
    expect(b.next()).toBe(1000);
  });
});

describe("classifyQuality", () => {
  it("good for clean conditions", () => {
    expect(classifyQuality({ packetLossFraction: 0, jitterMs: 5 })).toBe("good");
  });
  it("ok for moderate jitter", () => {
    expect(classifyQuality({ packetLossFraction: 0.01, jitterMs: 60 })).toBe(
      "ok",
    );
  });
  it("ok for moderate loss", () => {
    expect(
      classifyQuality({ packetLossFraction: 0.03, jitterMs: 20 }),
    ).toBe("ok");
  });
  it("bad for high loss", () => {
    expect(classifyQuality({ packetLossFraction: 0.1, jitterMs: 5 })).toBe(
      "bad",
    );
  });
});

describe("ActiveSpeakerTracker", () => {
  it("requires two consecutive windows before crowning", () => {
    const t = new ActiveSpeakerTracker();
    expect(t.push([{ pid: "A", level: 0.2 }])).toBe(null); // first window
    expect(t.push([{ pid: "A", level: 0.3 }])).toBe("A");  // confirmed
  });
  it("does not flicker on a single loud spike", () => {
    const t = new ActiveSpeakerTracker();
    t.push([{ pid: "A", level: 0.4 }]);
    t.push([{ pid: "A", level: 0.4 }]); // A is now winner
    expect(t.push([{ pid: "B", level: 0.5 }])).toBe("A"); // spike, not confirmed
    expect(t.push([{ pid: "B", level: 0.5 }])).toBe("B"); // confirmed
  });
});
