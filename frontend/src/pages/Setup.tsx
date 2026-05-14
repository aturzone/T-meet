import { useEffect, useState } from "react";

import { Card } from "../components/ui/Card";
import { Spinner } from "../components/ui/Spinner";
import { fetchSetupInfo } from "../lib/api";
import type { SetupInfo } from "../lib/schemas";

export default function Setup() {
  const [info, setInfo] = useState<SetupInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetchSetupInfo()
      .then((i) => {
        if (!cancelled) setInfo(i);
      })
      .catch((e) => {
        if (!cancelled) setError(e instanceof Error ? e.message : "failed");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <main className="min-h-screen flex items-center justify-center p-6">
      <Card className="max-w-2xl space-y-6">
        <header className="space-y-1">
          <h1 className="text-2xl font-semibold">Trust the local CA</h1>
          <p className="text-sm text-muted">
            Each device that joins a meeting needs to trust this server's
            certificate authority once. After that, no browser warnings.
          </p>
        </header>

        <section className="space-y-2">
          <h2 className="text-sm font-medium">1. Download the CA cert</h2>
          <p className="text-sm text-muted">
            <a
              className="text-accent underline"
              href={info?.ca_cert_url ?? "/ca.crt"}
              download="meet-ca.crt"
            >
              meet-ca.crt
            </a>
          </p>
        </section>

        <section className="space-y-2">
          <h2 className="text-sm font-medium">2. Trust it on your device</h2>
          <ul className="text-sm text-muted list-disc pl-5 space-y-1">
            <li>
              <strong className="text-fg">Linux (Debian / Ubuntu):</strong>{" "}
              <code>sudo cp meet-ca.crt /usr/local/share/ca-certificates/</code>
              {" "}then <code>sudo update-ca-certificates</code>.
            </li>
            <li>
              <strong className="text-fg">macOS:</strong> double-click the file,
              add to <em>System</em> keychain, set trust to{" "}
              <em>Always Trust</em>.
            </li>
            <li>
              <strong className="text-fg">Windows:</strong> double-click,{" "}
              <em>Install Certificate → Local Machine → Trusted Root</em>.
            </li>
            <li>
              <strong className="text-fg">Brave / Chromium / Firefox:</strong>{" "}
              if your browser uses its own trust store, import the cert into
              that store explicitly.
            </li>
          </ul>
        </section>

        <section className="space-y-2">
          <h2 className="text-sm font-medium">3. Verify the fingerprint</h2>
          <p className="text-sm text-muted">
            Compare the SHA-256 your browser shows against the value below:
          </p>
          {info && (
            <code className="block text-xs bg-bg border border-border rounded px-3 py-2 break-all">
              {info.leaf_fingerprint_sha256}
            </code>
          )}
          {!info && !error && (
            <div className="flex items-center gap-2 text-xs text-muted">
              <Spinner className="size-4" /> Loading fingerprint…
            </div>
          )}
          {error && (
            <p className="text-xs text-red-400">Fingerprint unavailable.</p>
          )}
        </section>
      </Card>
    </main>
  );
}
