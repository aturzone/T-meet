#![allow(clippy::disallowed_methods)]

//! Phase 02 acceptance.
//!
//! - HTTPS listener serves /healthz, /ca.crt, and the embedded SPA shell.
//! - Plain HTTP listener 301s non-/ca.crt requests to HTTPS.
//! - Plain HTTP /ca.crt still works (locked-down-device escape hatch).
//! - DB file is created with mode 0600 and the migration set is applied.
//! - x-request-id appears on every response.
//! - Security headers on every response.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use meet_core::config::{Config, LogConfig, LogFormat, ServerConfig, StorageConfig};
use meet_core::db::Db;
use meet_server::run_init_for_tests;
use meet_server::run_serve_for_tests;
use secrecy::SecretBox;

fn pp(s: &str) -> SecretBox<String> {
    SecretBox::new(Box::new(s.to_owned()))
}

fn pick_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

fn cfg_in_temp(dir: &std::path::Path, tls_port: u16, http_port: u16) -> Config {
    Config {
        server: ServerConfig {
            bind_ip: "127.0.0.1".parse().unwrap(),
            tls_port,
            http_redirect_port: http_port,
            external_host: None,
        },
        storage: StorageConfig {
            data_dir: PathBuf::from(dir),
        },
        log: LogConfig {
            level: "warn".into(),
            format: LogFormat::Pretty,
        },
    }
}

async fn wait_tcp(addr: SocketAddr) {
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("nothing listening at {addr}");
}

fn https_client(ca_pem: &str) -> reqwest::Client {
    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls_pemfile::certs(&mut ca_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    {
        roots.add(cert).unwrap();
    }
    meet_server::install_tls_provider_for_tests();
    let tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    reqwest::Client::builder()
        .use_preconfigured_tls(tls)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread")]
async fn serve_brings_up_https_and_http_with_db() {
    let tmp = tempfile::tempdir().unwrap();
    let tls_port = pick_port();
    let http_port = pick_port();
    let cfg = cfg_in_temp(tmp.path(), tls_port, http_port);
    run_init_for_tests(&cfg, &pp("correct horse battery staple")).expect("init");

    let ca_pem = std::fs::read_to_string(tmp.path().join("ca.crt")).unwrap();
    let cfg_for_serve = cfg.clone();
    let server = tokio::spawn(async move {
        let _ = run_serve_for_tests(cfg_for_serve, pp("correct horse battery staple")).await;
    });

    wait_tcp(format!("127.0.0.1:{tls_port}").parse().unwrap()).await;
    wait_tcp(format!("127.0.0.1:{http_port}").parse().unwrap()).await;

    // --- DB file checks ----------------------------------------------------
    let db_path = tmp.path().join("meet.db");
    assert!(db_path.exists(), "meet.db should be created on serve");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&db_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "meet.db must be chmod 0600");
    }

    // --- HTTPS surface -----------------------------------------------------
    let client = https_client(&ca_pem);
    let healthz = client
        .get(format!("https://127.0.0.1:{tls_port}/healthz"))
        .send()
        .await
        .unwrap();
    assert_eq!(healthz.status(), 200);
    // Headers we promised in Phase 02.
    assert!(healthz.headers().get("x-request-id").is_some());
    assert_eq!(
        healthz.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert_eq!(
        healthz.headers().get("referrer-policy").unwrap(),
        "no-referrer"
    );
    assert!(healthz
        .headers()
        .get("content-security-policy")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("default-src 'self'"));

    // /ca.crt over HTTPS.
    let ca = client
        .get(format!("https://127.0.0.1:{tls_port}/ca.crt"))
        .send()
        .await
        .unwrap();
    assert_eq!(ca.status(), 200);
    assert!(ca
        .text()
        .await
        .unwrap()
        .starts_with("-----BEGIN CERTIFICATE-----"));

    // --- HTTP redirect -----------------------------------------------------
    let http_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let redirected = http_client
        .get(format!("http://127.0.0.1:{http_port}/anything"))
        .send()
        .await
        .unwrap();
    assert_eq!(redirected.status(), 301);
    let location = redirected
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        location.starts_with(&format!("https://127.0.0.1:{tls_port}/anything")),
        "got: {location}"
    );

    // /ca.crt over plain HTTP is the documented escape hatch.
    let ca_plain = http_client
        .get(format!("http://127.0.0.1:{http_port}/ca.crt"))
        .send()
        .await
        .unwrap();
    assert_eq!(ca_plain.status(), 200);
    assert!(ca_plain
        .text()
        .await
        .unwrap()
        .starts_with("-----BEGIN CERTIFICATE-----"));

    // --- SPA fallback ------------------------------------------------------
    let spa = client
        .get(format!("https://127.0.0.1:{tls_port}/some/route"))
        .send()
        .await
        .unwrap();
    assert_eq!(spa.status(), 200);
    assert!(spa
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/html"));

    // Reserved prefix returns a real 404, not the SPA shell.
    let api_404 = client
        .get(format!("https://127.0.0.1:{tls_port}/api/whatever"))
        .send()
        .await
        .unwrap();
    assert_eq!(api_404.status(), 404);

    server.abort();
}

#[tokio::test]
async fn open_creates_chmod_0600_db_file() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("nested/dir/meet.db");
    let db = Db::open(&path).await.expect("open");
    assert!(path.exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    drop(db);
}
