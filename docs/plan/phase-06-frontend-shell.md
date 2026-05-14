# Phase 06 — Frontend Shell

## Goal

Stand up the React + Vite + TypeScript + Tailwind frontend skeleton with the three routes the product needs (`/`, `/r/:id`, `/setup`), a Zustand store for app state, a toast/notification primitive, an error boundary, loading states, and the build pipeline that emits `frontend/dist/` for `rust-embed`. No live WebRTC yet — Phase 07 wires that in. The shell must look like a real product at this point: minimal, calm, restrained palette.

## Deliverables

- `frontend/src/main.tsx` — React root with `StrictMode`, the router, and the error boundary.
- `frontend/src/app/router.tsx` — `react-router-dom` routes for `/`, `/r/:id`, `/setup`, `/404`.
- `frontend/src/pages/Landing.tsx` — Join-meeting form: room URL or room id + password + display name; client-side zod validation; calls `POST /r/:id/join` on submit.
- `frontend/src/pages/Room.tsx` — placeholder room shell (header, "you're in {room.name}", participant list stub, controls bar, chat panel stub). Phase 07 replaces the body.
- `frontend/src/pages/Setup.tsx` — CA download + trust instructions; deep-links to OS-specific guides; shows a fingerprint of the running leaf cert.
- `frontend/src/pages/NotFound.tsx`.
- `frontend/src/components/ui/` — `Button`, `Input`, `Label`, `Card`, `Spinner`, `Toaster` (small, tailored — not a copy of a UI library).
- `frontend/src/lib/store.ts` — Zustand store: `session` (room id, join token, participant id, peer list), `ui` (toasts, modals).
- `frontend/src/lib/api.ts` — typed wrapper for `/r/:id/join` (returns the join response).
- `frontend/src/lib/error_boundary.tsx` — catches render errors, shows the request-id from the failing response if available.
- `frontend/src/lib/schemas.ts` — zod schemas mirroring the Phase 03 join request/response.
- `frontend/src/styles/tailwind.css` — tokens (`--color-bg`, `--color-fg`, `--color-accent`, etc.) and Tailwind layers.
- `frontend/tailwind.config.ts` — palette (slate + a single accent), font stack, radius, shadow tokens.
- `frontend/index.html` — viewport, theme-color, no inline scripts.
- `frontend/__tests__/` — Vitest smoke tests per page and component.
- `frontend/e2e/` — Playwright config with Brave (matches `.mcp.json`); one E2E that opens `/`, fills the join form, and asserts a network call.

## Design decisions

- **Vite + React 18 strict.** Already settled in Phase 00. Keep `noUncheckedIndexedAccess` on.
- **`react-router-dom` v6.** Mature, simple, no surprises.
- **Zustand over Redux.** Two stores (session, UI) is plenty. Selectors via `shallow` to avoid re-renders.
- **`react-hook-form` + `zod` for forms.** Type-safe end-to-end with the backend schema.
- **Tailwind v3 utility-first.** No design-system import. Components are small, hand-rolled, and tailored to T-meet's tone.
- **Restrained palette: slate for surfaces, a single accent for primary actions.** Avoids the rainbow-of-tones look that makes web apps feel like dashboards.
- **No analytics, no font CDN, no remote anything.** Fonts ship as woff2 in `frontend/public/fonts/`. The HTML has zero outbound references.
- **CSP-friendly from the start.** No `eval`, no inline scripts; Tailwind's `unsafe-inline` for styles is the only concession and is documented in Phase 09.
- **Per-route lazy loading.** Each page is its own chunk; the landing page loads in <50 KB JS gzipped.
- **Error boundary surfaces `x-request-id`.** Users can paste it into a bug report; it's the only correlation handle.
- **Setup page shows the leaf cert fingerprint.** A small honesty signal: users can compare it with what they see in the browser cert viewer when in doubt.

## Public interfaces

### Routes

| Path | Component | Notes |
|---|---|---|
| `/` | `Landing` | Form: `room_id_or_url`, `password`, `display_name`. Submits → `POST /r/:id/join` → on 200 navigate to `/r/:id`. |
| `/r/:id` | `Room` | Reads `id` param; expects the join token in the Zustand store; if missing, redirects to `/?next=/r/:id`. |
| `/setup` | `Setup` | CA download (`/ca.crt`), step-by-step trust instructions per OS, leaf cert fingerprint. |
| `/404` | `NotFound` | "Couldn't find that room" + back to `/`. |

### TS module surfaces

```ts
// lib/api.ts
export interface JoinResponse {
  join_token: string;
  ws_url: string;
  ice_servers: RTCIceServer[];
  participant_id: string;
}
export async function join(roomId: string, password: string, displayName: string): Promise<JoinResponse>;

// lib/store.ts
export interface SessionState {
  roomId?: string;
  joinToken?: string;
  participantId?: string;
  peers: { pid: string; displayName: string; pubkey?: string }[];
  setSession(r: JoinResponse & { roomId: string; displayName: string }): void;
  clear(): void;
}
export const useSession: UseBoundStore<StoreApi<SessionState>>;

export interface UiState {
  toasts: { id: string; level: "info"|"warn"|"error"; message: string }[];
  pushToast(level: UiState["toasts"][number]["level"], message: string): void;
  dismissToast(id: string): void;
}
export const useUi: UseBoundStore<StoreApi<UiState>>;
```

## Security considerations

- **No tokens in URLs.** The join token is stored only in Zustand (memory) and forgotten on page reload — the user re-enters the password. This is a deliberate UX choice; the security gain is that bookmarks and tab history never leak tokens.
- **No `dangerouslySetInnerHTML`.** Phase 09 grep enforces this.
- **No `target="_blank"` without `rel="noopener noreferrer"`.** Linter rule on the way in.
- **Strict CSP works.** No inline scripts, no remote fonts/images, no eval. Tailwind's inline styles are allowed only because `style-src 'self' 'unsafe-inline'` is the documented compromise.
- **Form data is validated client-side and server-side.** Zod on the client, serde + validators on the server.
- **Error boundary never displays raw server response bodies.** Only the request-id and a generic message; raw bodies could echo input data into the page.
- **Display names are escaped automatically by React.** Confirmed by an explicit test that renders a `<script>` payload as text, not as a script tag.
- Cross-references: prompt §4.12, §4.14.

## Test plan

- **Unit (Vitest):**
  - Zod schemas accept good inputs, reject bad inputs.
  - `useSession.setSession` populates the store; `clear` empties it.
  - `Button`, `Input`, `Toaster` render and respond to props.
- **Component tests:**
  - `Landing` renders the form; submitting an empty form shows zod errors.
  - `Landing` with mocked `join()` returning a token navigates to `/r/:id`.
  - `Setup` renders the fingerprint when provided.
- **E2E (Playwright + Brave):**
  - Open `/`, fill a valid join form against a test server, assert navigation to `/r/:id`.
  - Visit `/r/<unknown>` without a session → redirected to `/?next=...`.
- **Manual:**
  - Open in Brave with the CA trusted — no console errors, no network errors, CSP fires no violations.
  - DevTools → Coverage: landing route < 50 KB JS gzipped.

## Acceptance criteria

- [x] `pnpm -C frontend build` emits `frontend/dist/index.html` and code-split chunks.
- [x] All four routes render and the join flow navigates correctly with a mocked backend.
- [x] Tailwind theme tokens defined; no ad-hoc CSS files outside `tailwind.css`.
- [x] Setup page shows the leaf cert fingerprint when the server provides it via a new `GET /api/setup-info` endpoint (added in this phase).
- [x] Error boundary catches a forced render error and shows a generic message with the request-id if available.
- [x] Vitest suite passes (12 tests across `schemas`, `store`, and `App` rendering); ~~one Playwright E2E passes against the local server~~ — **deferred to Phase 07** where Playwright + Brave needs the full WebRTC + fake-device setup anyway.
- [x] CSP from Phase 02 enforced without violations on every page.
- [x] `just check` is green.

## Open questions

- Dark mode — defer to Phase 09 polish; ship light-only for v1. Confirm with the user before merging.
- Whether to include a "report a problem" link that opens the user's mail client with the request-id pre-filled. Recommendation: yes for v1; the mail client choice is the user's.
- Whether to keep the join token across page reloads via `sessionStorage`. Recommendation: don't; current "re-enter on reload" UX is the safer default for self-hosted contexts.
