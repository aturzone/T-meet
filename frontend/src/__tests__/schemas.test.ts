import { describe, it, expect } from "vitest";
import {
  extractRoomId,
  joinRequestSchema,
  joinResponseSchema,
  setupInfoSchema,
} from "../lib/schemas";

describe("extractRoomId", () => {
  it("accepts a bare id", () => {
    expect(extractRoomId("Xh3Abc123_-")).toBe("Xh3Abc123_-");
  });
  it("accepts an /r/<id> URL", () => {
    expect(extractRoomId("https://meet.local:8443/r/Xh3Abc123")).toBe(
      "Xh3Abc123",
    );
  });
  it("rejects gibberish", () => {
    expect(extractRoomId("    ")).toBeNull();
    expect(extractRoomId("with spaces")).toBeNull();
  });
});

describe("joinRequestSchema", () => {
  it("accepts a valid payload", () => {
    expect(
      joinRequestSchema.safeParse({
        password: "openSesame",
        display_name: "Alice",
      }).success,
    ).toBe(true);
  });
  it("rejects control characters in display_name", () => {
    expect(
      joinRequestSchema.safeParse({
        password: "x",
        display_name: "Alice",
      }).success,
    ).toBe(false);
  });
  it("rejects long display_name", () => {
    expect(
      joinRequestSchema.safeParse({
        password: "x",
        display_name: "a".repeat(65),
      }).success,
    ).toBe(false);
  });
});

describe("joinResponseSchema", () => {
  it("parses a server response", () => {
    const parsed = joinResponseSchema.parse({
      join_token: "v4.local.abc",
      ws_url: "/ws/Xh3",
      ice_servers: [],
      participant_id: "p1",
    });
    expect(parsed.join_token).toBe("v4.local.abc");
  });
});

describe("setupInfoSchema", () => {
  it("parses a setup info response", () => {
    const parsed = setupInfoSchema.parse({
      leaf_fingerprint_sha256: "AA:BB:CC",
      ca_cert_url: "/ca.crt",
    });
    expect(parsed.ca_cert_url).toBe("/ca.crt");
  });
});
