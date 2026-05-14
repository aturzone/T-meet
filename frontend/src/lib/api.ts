import {
  joinResponseSchema,
  setupInfoSchema,
  type JoinResponse,
  type SetupInfo,
} from "./schemas";

export class ApiError extends Error {
  readonly status: number;
  readonly requestId: string | null;
  constructor(message: string, status: number, requestId: string | null) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.requestId = requestId;
  }
}

export async function joinRoom(
  roomId: string,
  password: string,
  displayName: string,
): Promise<JoinResponse> {
  const resp = await fetch(`/r/${encodeURIComponent(roomId)}/join`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ password, display_name: displayName }),
  });
  if (!resp.ok) {
    throw apiError(resp);
  }
  const body = (await resp.json()) as unknown;
  const parsed = joinResponseSchema.safeParse(body);
  if (!parsed.success) {
    throw new ApiError("unexpected response shape", 502, resp.headers.get("x-request-id"));
  }
  return parsed.data;
}

export async function fetchSetupInfo(): Promise<SetupInfo> {
  const resp = await fetch("/api/setup-info");
  if (!resp.ok) {
    throw apiError(resp);
  }
  const body = (await resp.json()) as unknown;
  const parsed = setupInfoSchema.safeParse(body);
  if (!parsed.success) {
    throw new ApiError("unexpected response shape", 502, resp.headers.get("x-request-id"));
  }
  return parsed.data;
}

function apiError(resp: Response): ApiError {
  const message =
    resp.status === 401
      ? "invalid credentials"
      : resp.status === 429
        ? "too many attempts — try again later"
        : `request failed (${resp.status})`;
  return new ApiError(message, resp.status, resp.headers.get("x-request-id"));
}
