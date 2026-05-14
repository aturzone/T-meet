# Trusting the T-meet CA

T-meet ships its own per-install certificate authority. Each device that
joins a meeting needs to trust this CA exactly once; after that, no
browser warnings.

Get the CA cert from:

```
https://<host>:<tls-port>/ca.crt
```

(Plain HTTP works too as a fallback for locked-down devices:
`http://<host>:<http-port>/ca.crt`.)

Compare the SHA-256 fingerprint shown by your browser against the value
printed at first boot, or fetched from
`https://<host>:<tls-port>/api/setup-info`.

## Linux (Debian / Ubuntu)

```
sudo cp meet-ca.crt /usr/local/share/ca-certificates/meet-ca.crt
sudo update-ca-certificates
```

This trusts the cert system-wide. Restart any browser that was already
open.

### Linux (Fedora / RHEL / Arch)

```
sudo cp meet-ca.crt /etc/pki/ca-trust/source/anchors/    # Fedora/RHEL
sudo update-ca-trust extract
```

```
sudo cp meet-ca.crt /etc/ca-certificates/trust-source/anchors/   # Arch
sudo trust extract-compat
```

## macOS

```
sudo security add-trusted-cert -d -r trustRoot \
  -k /Library/Keychains/System.keychain meet-ca.crt
```

Or via the GUI:

1. Double-click `meet-ca.crt`. Keychain Access opens.
2. Move the entry to the **System** keychain.
3. Double-click the cert, expand **Trust**, set **Secure Sockets Layer
   (SSL)** to **Always Trust**.
4. Close and authenticate when prompted.

## Windows

1. Double-click `meet-ca.crt`.
2. Click **Install Certificate**.
3. Choose **Local Machine** (needs admin), click **Next**.
4. Choose **Place all certificates in the following store**, click
   **Browse**, pick **Trusted Root Certification Authorities**, click
   **OK**.
5. Finish.

## iOS / iPadOS

1. AirDrop or email the `meet-ca.crt` file to the device.
2. Open it; iOS prompts to download a profile.
3. Settings → General → VPN & Device Management → install the profile.
4. Settings → General → About → Certificate Trust Settings → toggle
   the meet CA to **enable full trust**.

## Android

1. Copy `meet-ca.crt` to the device.
2. Settings → Security → Install from storage → CA certificate.
3. Pick the file. Confirm. Set name "meet-ca".

Apps that opt out of the user CA store (most banking apps) will still
refuse the cert. The meeting app in any modern browser will accept it.

## Per-browser stores

Firefox keeps its own trust store separate from the OS. After importing
the CA into the OS:

- **Firefox:** Preferences → Privacy & Security → View Certificates →
  Authorities → Import → pick `meet-ca.crt` → enable "Trust this CA to
  identify websites".
- **Brave / Chrome / Edge / Chromium:** use the OS trust store on Linux
  + macOS + Windows by default; no extra step needed. On Linux,
  ensure `libnss3-tools` is installed and the NSS database has the
  cert (Brave doesn't always pick up the system anchor); the simplest
  workaround is to run Brave once with `--ignore-certificate-errors`
  to confirm the install path, then revert.

## Removing the CA

Each OS has an undo for the steps above. The Linux flow is just
`sudo rm /usr/local/share/ca-certificates/meet-ca.crt && sudo
update-ca-certificates`. Removing the cert immediately stops the
device from trusting *any* meeting hosted by this install.

## Why a private CA at all?

Browsers refuse to enable WebRTC (mic / camera APIs) on `http://`. A
private CA gives us TLS without depending on a public-internet CA, an
external DNS name, or LetsEncrypt's ACME challenges — all of which
would tie the deploy to internet reachability. T-meet stays
self-contained.
