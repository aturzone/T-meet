#![allow(clippy::disallowed_methods)]

//! Phase 03 acceptance: admin endpoints + public join.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use meet_core::config::{Config, LogConfig, LogFormat, ServerConfig, StorageConfig};
use meet_server::{run_init_for_tests, run_serve_for_tests, InitOutput};
use secrecy::SecretBox;
use serde_json::Value;

fn pp(s: &str) -> SecretBox<String> {
    SecretBox::new(Box::new(s.to_owned()))
}

fn pick_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

fn cfg(dir: &std::path::Path, tls: u16, http: u16) -> Config {
    Config {
        server: ServerConfig {
            bind_ip: "127.0.0.1".parse().unwrap(),
            tls_port: tls,
            http_redirect_port: http,
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

struct Started {
    out: InitOutput,
    ca_pem: String,
    tls_port: u16,
    handle: tokio::task::JoinHandle<()>,
}

async fn start_server(tmp: &std::path::Path) -> Started {
    let tls = pick_port();
    let http = pick_port();
    let cfg_init = cfg(tmp, tls, http);
    let out = run_init_for_tests(&cfg_init, &pp("correct horse battery staple")).expect("init");
    let ca_pem = std::fs::read_to_string(tmp.join("ca.crt")).unwrap();
    let cfg_serve = cfg_init.clone();
    let handle = tokio::spawn(async move {
        let _ = run_serve_for_tests(cfg_serve, pp("correct horse battery staple")).await;
    });
    wait_tcp(format!("127.0.0.1:{tls}").parse().unwrap()).await;
    Started {
        out,
        ca_pem,
        tls_port: tls,
        handle,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn admin_create_list_get_delete_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let s = start_server(tmp.path()).await;
    let client = https_client(&s.ca_pem);

    // Unauthenticated → 401.
    let r = client
        .post(format!("https://127.0.0.1:{}/admin/rooms", s.tls_port))
        .json(&serde_json::json!({"name": "x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);

    // Authenticated create.
    let create = client
        .post(format!("https://127.0.0.1:{}/admin/rooms", s.tls_port))
        .bearer_auth(&s.out.admin_token)
        .json(&serde_json::json!({"name": "All-Hands"}))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);
    let body: Value = create.json().await.unwrap();
    let room_id = body["id"].as_str().unwrap().to_owned();
    let password = body["password"].as_str().unwrap().to_owned();
    assert!(!password.is_empty(), "server must generate a password");
    assert_eq!(body["name"], "All-Hands");
    assert!(body["join_url"].as_str().unwrap().contains(&room_id));

    // Wrong bearer → 401.
    let wrong = client
        .get(format!("https://127.0.0.1:{}/admin/rooms", s.tls_port))
        .bearer_auth("v4.local.AAAAAAA")
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 401);

    // List sees the new room.
    let list = client
        .get(format!("https://127.0.0.1:{}/admin/rooms", s.tls_port))
        .bearer_auth(&s.out.admin_token)
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), 200);
    let body: Value = list.json().await.unwrap();
    let rooms = body["rooms"].as_array().unwrap();
    assert!(rooms.iter().any(|r| r["id"] == room_id));

    // Get the single room.
    let one = client
        .get(format!(
            "https://127.0.0.1:{}/admin/rooms/{}",
            s.tls_port, room_id
        ))
        .bearer_auth(&s.out.admin_token)
        .send()
        .await
        .unwrap();
    assert_eq!(one.status(), 200);

    // Delete → 204.
    let del = client
        .delete(format!(
            "https://127.0.0.1:{}/admin/rooms/{}",
            s.tls_port, room_id
        ))
        .bearer_auth(&s.out.admin_token)
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);

    // Second delete → 404.
    let again = client
        .delete(format!(
            "https://127.0.0.1:{}/admin/rooms/{}",
            s.tls_port, room_id
        ))
        .bearer_auth(&s.out.admin_token)
        .send()
        .await
        .unwrap();
    assert_eq!(again.status(), 404);

    s.handle.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn join_flow_right_and_wrong_passwords_and_rate_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let s = start_server(tmp.path()).await;
    let client = https_client(&s.ca_pem);

    // Create a room with a known password.
    let create = client
        .post(format!("https://127.0.0.1:{}/admin/rooms", s.tls_port))
        .bearer_auth(&s.out.admin_token)
        .json(&serde_json::json!({"name": "Sprint review", "password": "openSesame!"}))
        .send()
        .await
        .unwrap();
    let body: Value = create.json().await.unwrap();
    let room_id = body["id"].as_str().unwrap().to_owned();

    // Wrong password → 401 generic.
    let bad = client
        .post(format!(
            "https://127.0.0.1:{}/r/{}/join",
            s.tls_port, room_id
        ))
        .json(&serde_json::json!({"password": "wrong", "display_name": "Alice"}))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 401);
    let bad_body: Value = bad.json().await.unwrap();
    assert_eq!(bad_body["error"], "invalid credentials");

    // Unknown room → 401 generic (NOT 404).
    let unknown = client
        .post(format!(
            "https://127.0.0.1:{}/r/nosuchroom1234567890/join",
            s.tls_port
        ))
        .json(&serde_json::json!({"password": "x", "display_name": "Alice"}))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown.status(), 401);

    // Right password → 200 with join_token + participant_id.
    let ok = client
        .post(format!(
            "https://127.0.0.1:{}/r/{}/join",
            s.tls_port, room_id
        ))
        .json(&serde_json::json!({"password": "openSesame!", "display_name": "Alice"}))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);
    let body: Value = ok.json().await.unwrap();
    let token = body["join_token"].as_str().unwrap().to_owned();
    assert!(token.starts_with("v4.local."), "got: {token}");
    assert_eq!(body["ws_url"], format!("/ws/{room_id}"));
    assert!(body["participant_id"].as_str().unwrap().len() >= 22);

    // Rate limit is scoped to (ip, room_id). On this specific room we've
    // used 2 attempts so far (bad password + good join). 3 more attempts to
    // hit the cap of 5, 6th must be 429.
    for _ in 0..3 {
        let r = client
            .post(format!(
                "https://127.0.0.1:{}/r/{}/join",
                s.tls_port, room_id
            ))
            .json(&serde_json::json!({"password": "wrong", "display_name": "Alice"}))
            .send()
            .await
            .unwrap();
        // attempts #3-5 should still authenticate-fail with 401.
        assert_eq!(r.status(), 401);
    }
    let too_many = client
        .post(format!(
            "https://127.0.0.1:{}/r/{}/join",
            s.tls_port, room_id
        ))
        .json(&serde_json::json!({"password": "openSesame!", "display_name": "Alice"}))
        .send()
        .await
        .unwrap();
    assert_eq!(too_many.status(), 429);
    assert!(too_many.headers().get("retry-after").is_some());

    s.handle.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn admin_token_verifies_against_persisted_secret_after_restart() {
    let tmp = tempfile::tempdir().unwrap();

    // First boot: keep the admin token.
    let cfg1 = cfg(tmp.path(), pick_port(), pick_port());
    let out = run_init_for_tests(&cfg1, &pp("correct horse battery staple")).expect("init");
    let admin_token = out.admin_token;

    // Re-derive the secret on serve(): the same passphrase decrypts admin.bin.
    let ca_pem = std::fs::read_to_string(tmp.path().join("ca.crt")).unwrap();
    let tls_port = pick_port();
    let http_port = pick_port();
    let cfg2 = cfg(tmp.path(), tls_port, http_port);
    let handle = tokio::spawn(async move {
        let _ = run_serve_for_tests(cfg2, pp("correct horse battery staple")).await;
    });
    wait_tcp(format!("127.0.0.1:{tls_port}").parse().unwrap()).await;

    let client = https_client(&ca_pem);
    let r = client
        .get(format!("https://127.0.0.1:{tls_port}/admin/rooms"))
        .bearer_auth(&admin_token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    handle.abort();
}
