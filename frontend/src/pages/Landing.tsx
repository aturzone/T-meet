import { forwardRef, useState, type InputHTMLAttributes } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { useNavigate } from "react-router-dom";
import { z } from "zod";

import { Button } from "../components/ui/Button";
import { Card } from "../components/ui/Card";
import { Input } from "../components/ui/Input";
import { Label } from "../components/ui/Label";
import { ApiError, joinRoom } from "../lib/api";
import { extractRoomId } from "../lib/schemas";
import { useSession, useUi } from "../lib/store";

const formSchema = z.object({
  room: z
    .string()
    .min(1, "room required")
    .refine((s) => extractRoomId(s) !== null, "use a room id or /r/<id> URL"),
  password: z.string().min(1, "password required").max(256),
  displayName: z
    .string()
    .min(1, "name required")
    .max(64)
    .refine(
      (s) => !Array.from(s).some((c) => c.charCodeAt(0) < 0x20),
      "no control characters",
    ),
});

type FormValues = z.infer<typeof formSchema>;

export default function Landing() {
  const navigate = useNavigate();
  const setSession = useSession((s) => s.setSession);
  const pushToast = useUi((s) => s.pushToast);
  const [submitting, setSubmitting] = useState(false);

  const {
    register,
    handleSubmit,
    formState: { errors },
  } = useForm<FormValues>({
    resolver: zodResolver(formSchema),
    defaultValues: { room: "", password: "", displayName: "" },
  });

  async function onSubmit(values: FormValues) {
    const roomId = extractRoomId(values.room);
    if (!roomId) {
      pushToast("error", "couldn't read that room id");
      return;
    }
    setSubmitting(true);
    try {
      const resp = await joinRoom(roomId, values.password, values.displayName);
      setSession({ ...resp, roomId, displayName: values.displayName });
      navigate(`/r/${roomId}`);
    } catch (e) {
      const msg = e instanceof ApiError ? e.message : "join failed";
      pushToast("error", msg);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <main className="min-h-screen flex items-center justify-center p-6">
      <Card className="w-full max-w-md space-y-5">
        <header className="space-y-1">
          <h1 className="text-2xl font-semibold">Join a meeting</h1>
          <p className="text-sm text-muted">
            Paste the room link or room id from your host.
          </p>
        </header>

        <form
          onSubmit={handleSubmit(onSubmit)}
          className="space-y-4"
          noValidate
        >
          <Field
            id="room"
            label="Room"
            placeholder="https://meet.local:8443/r/Xh3… or Xh3…"
            error={errors.room?.message}
            {...register("room")}
          />
          <Field
            id="password"
            label="Room password"
            type="password"
            autoComplete="off"
            error={errors.password?.message}
            {...register("password")}
          />
          <Field
            id="displayName"
            label="Your name"
            placeholder="Alice"
            error={errors.displayName?.message}
            {...register("displayName")}
          />
          <Button type="submit" loading={submitting} className="w-full">
            Join
          </Button>
        </form>

        <p className="text-xs text-muted text-center">
          First visit?{" "}
          <a className="text-accent underline" href="/setup">
            Trust the local CA
          </a>
          .
        </p>
      </Card>
    </main>
  );
}

interface FieldProps extends InputHTMLAttributes<HTMLInputElement> {
  id: string;
  label: string;
  error?: string | undefined;
}

const Field = forwardRef<HTMLInputElement, FieldProps>(function Field(
  { id, label, error, ...rest },
  ref,
) {
  return (
    <div className="space-y-1">
      <Label htmlFor={id}>{label}</Label>
      <Input
        ref={ref}
        id={id}
        aria-invalid={!!error}
        invalid={!!error}
        {...rest}
      />
      {error && <p className="text-xs text-red-400">{error}</p>}
    </div>
  );
});
