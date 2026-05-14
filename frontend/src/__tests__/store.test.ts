import { describe, it, expect, beforeEach } from "vitest";
import { useSession, useUi } from "../lib/store";

beforeEach(() => {
  useSession.getState().clear();
  // Wipe toasts.
  const ui = useUi.getState();
  for (const t of ui.toasts) {
    ui.dismissToast(t.id);
  }
});

describe("useSession", () => {
  it("sets and clears the session", () => {
    useSession.getState().setSession({
      join_token: "v4.local.abc",
      ws_url: "/ws/r1",
      ice_servers: [],
      participant_id: "p1",
      roomId: "r1",
      displayName: "Alice",
    });
    expect(useSession.getState().joinToken).toBe("v4.local.abc");
    expect(useSession.getState().displayName).toBe("Alice");

    useSession.getState().clear();
    expect(useSession.getState().joinToken).toBeUndefined();
    expect(useSession.getState().peers).toHaveLength(0);
  });

  it("adds and removes peers idempotently", () => {
    const s = useSession.getState();
    s.addPeer({ pid: "p1", displayName: "Alice" });
    s.addPeer({ pid: "p1", displayName: "Alice-dup" }); // duplicate ignored
    s.addPeer({ pid: "p2", displayName: "Bob" });
    expect(useSession.getState().peers).toHaveLength(2);

    s.removePeer("p1");
    expect(useSession.getState().peers.map((p) => p.pid)).toEqual(["p2"]);
  });
});

describe("useUi", () => {
  it("pushes and dismisses toasts", () => {
    useUi.getState().pushToast("error", "boom");
    expect(useUi.getState().toasts).toHaveLength(1);
    const id = useUi.getState().toasts[0]!.id;
    useUi.getState().dismissToast(id);
    expect(useUi.getState().toasts).toHaveLength(0);
  });
});
