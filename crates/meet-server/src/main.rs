#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use meet_core::config::Config;
use meet_core::log;
use meet_server::{init, passphrase, serve};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map_or("--help", String::as_str);

    match cmd {
        "--version" | "-V" => {
            println!("meet-server {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        },
        "--help" | "-h" => {
            print_help();
            ExitCode::SUCCESS
        },
        "init" => run_init_cmd(),
        "serve" => run_serve_cmd(),
        other => {
            eprintln!("unknown command: {other}");
            print_help();
            ExitCode::from(2)
        },
    }
}

fn run_init_cmd() -> ExitCode {
    let cfg = load_or_default();
    log::init(&cfg.log);
    let pp = match passphrase::read_admin_passphrase() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("passphrase error: {e}");
            return ExitCode::from(2);
        },
    };
    match init::run_init(&cfg, &pp) {
        Ok(out) => {
            print_first_boot_banner(&cfg, &out.leaf_fingerprint_sha256, &out.admin_token);
            ExitCode::SUCCESS
        },
        Err(e) => {
            tracing::error!(error = %e, "init failed");
            ExitCode::from(1)
        },
    }
}

fn run_serve_cmd() -> ExitCode {
    let cfg = load_or_default();
    log::init(&cfg.log);
    let pp = match passphrase::read_admin_passphrase() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("passphrase error: {e}");
            return ExitCode::from(2);
        },
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("tokio runtime: {e}");
            return ExitCode::from(1);
        },
    };

    match runtime.block_on(serve::run_serve(cfg, pp)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "serve failed");
            ExitCode::from(1)
        },
    }
}

fn load_or_default() -> Config {
    let path = PathBuf::from("config.toml");
    if path.exists() {
        match Config::load(&path) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("config load failed ({e}); using defaults");
                Config::default()
            },
        }
    } else {
        Config::default()
    }
}

fn print_first_boot_banner(cfg: &Config, leaf_fingerprint: &str, admin_token: &str) {
    let host = cfg
        .server
        .external_host
        .clone()
        .unwrap_or_else(|| cfg.server.bind_ip.to_string());
    let port = cfg.server.tls_port;
    println!();
    println!("================================================================");
    println!("T-meet — first-boot setup complete");
    println!();
    println!("  Admin token (save this — it is shown ONCE):");
    println!("    {admin_token}");
    println!();
    println!("  Download the CA cert and trust it on every device that will join:");
    println!("    https://{host}:{port}/ca.crt");
    println!();
    println!("  Send users this setup page:");
    println!("    https://{host}:{port}/setup");
    println!();
    println!("  Leaf cert SHA-256 fingerprint (compare with browser):");
    println!("    {leaf_fingerprint}");
    println!("================================================================");
    println!();
}

fn print_help() {
    println!(
        "meet-server {version}

USAGE:
    meet-server <COMMAND>

COMMANDS:
    init        One-time first-boot setup (generate CA + first leaf)
    serve       Run the server
    --version   Print version
    --help      Print this message

ENVIRONMENT:
    MEET_ADMIN_PASSPHRASE   Read instead of prompting on a TTY.
",
        version = env!("CARGO_PKG_VERSION")
    );
}
