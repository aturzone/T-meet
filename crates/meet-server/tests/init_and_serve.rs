#![allow(clippy::disallowed_methods)]

//! End-to-end Phase 01 acceptance test.
//!
//! 1. `run_init` against a temp dir produces ca.bin / ca.crt / leaf.pem / leaf.key
//!    with the right permissions and a parseable PEM.
//! 2. `run_serve` opens an HTTPS listener; an HTTP client with the CA trusted
//!    can fetch `/ca.crt` without certificate errors.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use meet_core::config::{Config, LogFormat, ServerConfig, StorageConfig};
use meet_server::run_init_for_tests;
use meet_server::run_serve_for_tests;
use secrecy::SecretBox;
use std::sync::Once;

static INIT_TRACING: Once = Once::new();

fn pp(s: &str) -> SecretBox<String> {
    SecretBox::new(Box::new(s.to_owned()))
}

fn pick_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    port
}

fn cfg_in_temp(dir: &std::path::Path, port: u16, redirect_port: u16) -> Config {
    Config {
        server: ServerConfig {
            bind_ip: "127.0.0.1".parse().unwrap(),
            tls_port: port,
            http_redirect_port: redirect_port,
            external_host: None,
        },
        storage: StorageConfig {
            data_dir: PathBuf::from(dir),
        },
        log: meet_core::config::LogConfig {
            level: "warn".into(),
            format: LogFormat::Pretty,
        },
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn init_writes_expected_files() {
    INIT_TRACING.call_once(|| {
        meet_core::log::init(&meet_core::config::LogConfig::default());
    });
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = cfg_in_temp(tmp.path(), 0, 1);
    let out = run_init_for_tests(&cfg, &pp("correct horse")).expect("init");

    let root = tmp.path();
    assert!(root.join("ca.bin").exists());
    assert!(root.join("ca.crt").exists());
    assert!(root.join("leaf.pem").exists());
    assert!(root.join("leaf.key").exists());

    let ca_pem = std::fs::read_to_string(root.join("ca.crt")).unwrap();
    assert!(ca_pem.starts_with("-----BEGIN CERTIFICATE-----"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let key_mode = std::fs::metadata(root.join("leaf.key"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(key_mode, 0o600, "leaf.key must be chmod 0600");
        let ca_mode = std::fs::metadata(root.join("ca.bin"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(ca_mode, 0o600, "ca.bin must be chmod 0600");
    }

    assert!(
        out.leaf_fingerprint_sha256.contains(':'),
        "fingerprint should be colon-separated hex"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn init_refuses_second_run() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = cfg_in_temp(tmp.path(), 0, 1);
    run_init_for_tests(&cfg, &pp("correct horse")).expect("init");
    let err = run_init_for_tests(&cfg, &pp("correct horse")).expect_err("second run should fail");
    assert!(format!("{err}").contains("refusing to overwrite"));
}

#[tokio::test(flavor = "multi_thread")]
async fn serve_returns_ca_crt_over_tls() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let port = pick_port();
    let redir = pick_port();
    let cfg = cfg_in_temp(tmp.path(), port, redir);
    let pp_init = pp("correct horse battery staple");
    run_init_for_tests(&cfg, &pp_init).expect("init");

    let ca_pem = std::fs::read_to_string(tmp.path().join("ca.crt")).unwrap();

    let cfg_for_serve = cfg.clone();
    let pp_for_serve = pp("correct horse battery staple");
    let server = tokio::spawn(async move {
        let _ = run_serve_for_tests(cfg_for_serve, pp_for_serve).await;
    });

    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    wait_for_tls(addr).await;

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
    let client = reqwest::Client::builder()
        .use_preconfigured_tls(tls)
        .build()
        .unwrap();

    let url = format!("https://127.0.0.1:{port}/ca.crt");
    let resp = client.get(&url).send().await.expect("get /ca.crt");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.starts_with("-----BEGIN CERTIFICATE-----"));

    let healthz = client
        .get(format!("https://127.0.0.1:{port}/healthz"))
        .send()
        .await
        .expect("get /healthz");
    assert_eq!(healthz.status(), 200);
    assert_eq!(healthz.text().await.unwrap(), "ok");

    server.abort();
}

async fn wait_for_tls(addr: SocketAddr) {
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("server never accepted connections at {addr}");
}
