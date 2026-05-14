import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import App from "../App";

describe("App", () => {
  it("renders the landing page heading", async () => {
    render(<App />);
    // Landing is lazy-loaded — wait for it.
    const heading = await screen.findByRole(
      "heading",
      { name: /join a meeting/i },
      { timeout: 2000 },
    );
    expect(heading).toBeTruthy();
  });
});
