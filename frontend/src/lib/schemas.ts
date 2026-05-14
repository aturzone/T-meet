import { z } from "zod";

export const joinRequestSchema = z.object({
  password: z.string().min(1, "password required").max(256),
  display_name: z
    .string()
    .min(1, "name required")
    .max(64, "name too long")
    .refine(
      (s) => !Array.from(s).some((c) => c.charCodeAt(0) < 0x20),
      "no control characters",
    ),
});

export type JoinRequest = z.infer<typeof joinRequestSchema>;

export const joinResponseSchema = z.object({
  join_token: z.string(),
  ws_url: z.string(),
  ice_servers: z.array(z.unknown()).default([]),
  participant_id: z.string(),
});

export type JoinResponse = z.infer<typeof joinResponseSchema>;

export const setupInfoSchema = z.object({
  leaf_fingerprint_sha256: z.string(),
  ca_cert_url: z.string(),
});

export type SetupInfo = z.infer<typeof setupInfoSchema>;

/** Accepts either a bare room id ("Xh3...") or a `/r/<id>` URL. */
export function extractRoomId(input: string): string | null {
  const trimmed = input.trim();
  if (!trimmed) return null;

  try {
    const u = new URL(trimmed);
    const m = u.pathname.match(/^\/r\/([A-Za-z0-9_-]{6,40})\/?$/);
    if (m && m[1]) return m[1];
  } catch {
    // not a URL — fall through
  }

  if (/^[A-Za-z0-9_-]{6,40}$/.test(trimmed)) {
    return trimmed;
  }
  return null;
}
