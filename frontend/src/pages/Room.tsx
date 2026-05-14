import { useEffect } from "react";
import { useNavigate, useParams } from "react-router-dom";

import { Button } from "../components/ui/Button";
import { Card } from "../components/ui/Card";
import { useSession } from "../lib/store";

export default function Room() {
  const params = useParams<{ id: string }>();
  const navigate = useNavigate();
  const session = useSession();

  useEffect(() => {
    if (!session.joinToken || session.roomId !== params.id) {
      navigate(`/?next=/r/${params.id ?? ""}`);
    }
  }, [navigate, params.id, session.joinToken, session.roomId]);

  if (!session.joinToken || session.roomId !== params.id) {
    return null;
  }

  return (
    <main className="min-h-screen flex flex-col p-4 gap-4">
      <header className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold">Room {session.roomId}</h1>
          <p className="text-xs text-muted">
            Joined as {session.displayName}
          </p>
        </div>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => {
            session.clear();
            navigate("/");
          }}
        >
          Leave
        </Button>
      </header>

      <Card className="flex-1 flex items-center justify-center">
        <div className="text-center text-muted space-y-2">
          <p className="text-sm">Video grid lands in Phase 07.</p>
          <p className="text-xs">
            Connected as participant {session.participantId?.slice(0, 8) ?? "?"}…
          </p>
        </div>
      </Card>
    </main>
  );
}
