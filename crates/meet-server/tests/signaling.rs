#![allow(
    clippy::disallowed_methods,
    clippy::useless_conversion,
    clippy::needless_continue
)]

//! Phase 04 acceptance: WS upgrade, first-message auth, peer fan-out, chat,
//! protocol-violation close codes.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use meet_core::config::{Config, LogConfig, LogFormat, ServerConfig, StorageConfig};
use meet_server::{run_init_for_tests, run_serve_for_tests};
use secrecy::SecretBox;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async_tls_with_config, Connector};

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

fn ws_tls_connector(ca_pem: &str) -> Connector {
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
    Connector::Rustls(Arc::new(tls))
}

struct Started {
    admin_token: String,
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
        admin_token: out.admin_token,
        ca_pem,
        tls_port: tls,
        handle,
    }
}

async fn create_room_and_join(s: &Started, name: &str, display_name: &str) -> (String, String) {
    let client = https_client(&s.ca_pem);
    let body: Value = client
        .post(format!("https://127.0.0.1:{}/admin/rooms", s.tls_port))
        .bearer_auth(&s.admin_token)
        .json(&json!({"name": name, "password": "openSesame!"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id = body["id"].as_str().unwrap().to_owned();
    let join: Value = client
        .post(format!(
            "https://127.0.0.1:{}/r/{}/join",
            s.tls_port, room_id
        ))
        .json(&json!({"password": "openSesame!", "display_name": display_name}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    (room_id, join["join_token"].as_str().unwrap().to_owned())
}

async fn open_ws(
    s: &Started,
    room_id: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let url = format!("wss://127.0.0.1:{}/ws/{}", s.tls_port, room_id);
    let connector = ws_tls_connector(&s.ca_pem);
    let (sock, _resp) = connect_async_tls_with_config(&url, None, false, Some(connector))
        .await
        .expect("ws connect");
    sock
}

async fn next_json(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Value {
    loop {
        match ws.next().await {
            Some(Ok(Message::Text(t))) => return serde_json::from_str(&t).expect("json"),
            Some(Ok(Message::Ping(p))) => {
                ws.send(Message::Pong(p)).await.ok();
            },
            Some(Ok(_)) => continue,
            Some(Err(e)) => panic!("ws error: {e}"),
            None => panic!("ws closed unexpectedly"),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn join_flow_and_peer_fanout() {
    let tmp = tempfile::tempdir().unwrap();
    let s = start_server(tmp.path()).await;

    let (room_id, token_a) = create_room_and_join(&s, "stand-up", "Alice").await;
    let (_room_id_b, token_b) = {
        // Same room, different display name. Join the *same* room id.
        let join: Value = https_client(&s.ca_pem)
            .post(format!(
                "https://127.0.0.1:{}/r/{}/join",
                s.tls_port, room_id
            ))
            .json(&json!({"password": "openSesame!", "display_name": "Bob"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        (
            room_id.clone(),
            join["join_token"].as_str().unwrap().to_owned(),
        )
    };

    let mut ws_a = open_ws(&s, &room_id).await;
    ws_a.send(Message::Text(
        json!({"type": "Join", "v": 1, "token": token_a})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let joined_a = next_json(&mut ws_a).await;
    assert_eq!(joined_a["type"], "Joined");
    assert!(joined_a["peers"].as_array().unwrap().is_empty());

    let mut ws_b = open_ws(&s, &room_id).await;
    ws_b.send(Message::Text(
        json!({"type": "Join", "v": 1, "token": token_b})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let joined_b = next_json(&mut ws_b).await;
    assert_eq!(joined_b["type"], "Joined");
    let peers = joined_b["peers"].as_array().unwrap();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0]["display_name"], "Alice");

    // Alice sees PeerJoined for Bob.
    let pj = next_json(&mut ws_a).await;
    assert_eq!(pj["type"], "PeerJoined");
    assert_eq!(pj["peer"]["display_name"], "Bob");

    // Chat fan-out: Alice → all, Bob receives ciphertext verbatim.
    ws_a.send(Message::Text(
        json!({
            "type": "Chat",
            "v": 1,
            "ciphertext": "AAAAAA",
            "nonce": "BBBBBB",
            "to": "all"
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let chat = next_json(&mut ws_b).await;
    assert_eq!(chat["type"], "Chat");
    assert_eq!(chat["ciphertext"], "AAAAAA");
    assert_eq!(chat["nonce"], "BBBBBB");
    // `from` is Alice's pid — opaque, but must be present.
    assert!(chat["from"].as_str().unwrap().len() >= 4);

    // Bob disconnects → Alice gets PeerLeft.
    ws_b.send(Message::Close(None)).await.ok();
    let pl = next_json(&mut ws_a).await;
    assert_eq!(pl["type"], "PeerLeft");

    s.handle.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn first_message_must_be_join_within_5s() {
    let tmp = tempfile::tempdir().unwrap();
    let s = start_server(tmp.path()).await;
    let (room_id, _) = create_room_and_join(&s, "auth-fail", "Alice").await;

    let mut ws = open_ws(&s, &room_id).await;
    // Send a non-Join first frame.
    ws.send(Message::Text(
        json!({"type": "Ping", "v": 1, "ts": 1}).to_string().into(),
    ))
    .await
    .unwrap();

    let frame = ws.next().await;
    match frame {
        Some(Ok(Message::Close(Some(cf)))) => assert_eq!(u16::from(cf.code), 4400),
        other => panic!("expected Close 4400, got {other:?}"),
    }

    s.handle.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_token_closes_4401() {
    let tmp = tempfile::tempdir().unwrap();
    let s = start_server(tmp.path()).await;
    let (room_id, _) = create_room_and_join(&s, "auth-fail-2", "Alice").await;

    let mut ws = open_ws(&s, &room_id).await;
    ws.send(Message::Text(
        json!({"type": "Join", "v": 1, "token": "v4.local.NOPE"})
            .to_string()
            .into(),
    ))
    .await
    .unwrap();

    let frame = ws.next().await;
    match frame {
        Some(Ok(Message::Close(Some(cf)))) => assert_eq!(u16::from(cf.code), 4401),
        other => panic!("expected Close 4401, got {other:?}"),
    }

    s.handle.abort();
}
