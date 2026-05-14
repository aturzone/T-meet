#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use meet_core::config::Config;
use meet_core::log;
use meet_server::{admin, init, passphrase, serve};

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
        "admin" => run_admin_cmd(&args),
        "doctor" => run_doctor_cmd(),
        other => {
            eprintln!("unknown command: {other}");
            print_help();
            ExitCode::from(2)
        },
    }
}

fn run_admin_cmd(args: &[String]) -> ExitCode {
    let sub = args.get(2).map_or("--help", String::as_str);
    let subsub = args.get(3).map_or("", String::as_str);
    match (sub, subsub) {
        ("token", "regenerate") => run_admin_token_regenerate(),
        ("status", _) => run_admin_status(),
        _ => {
            eprintln!("usage:\n  meet-server admin token regenerate\n  meet-server admin status");
            ExitCode::from(2)
        },
    }
}

fn run_admin_token_regenerate() -> ExitCode {
    let cfg = load_or_default();
    log::init(&cfg.log);
    let pp = match passphrase::read_admin_passphrase() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("passphrase error: {e}");
            return ExitCode::from(2);
        },
    };
    match admin::regenerate_admin_token(&cfg, &pp) {
        Ok(token) => {
            println!();
            println!("================================================================");
            println!("T-meet — new admin token (shown ONCE):");
            println!("    {token}");
            println!();
            println!("Outstanding admin tokens are now invalid.");
            println!("================================================================");
            ExitCode::SUCCESS
        },
        Err(e) => {
            eprintln!("admin token regenerate failed: {e}");
            ExitCode::from(1)
        },
    }
}

fn run_admin_status() -> ExitCode {
    let cfg = load_or_default();
    log::init(&cfg.log);
    match admin::status(&cfg) {
        Ok(s) => {
            println!("data_dir         {}", s.data_dir.display());
            println!("leaf valid from  {}", s.leaf_not_before);
            println!("leaf valid until {}", s.leaf_not_after);
            println!("leaf days left   {}", s.leaf_days_remaining);
            println!("rooms            {}", s.rooms);
            println!("audit entries    {}", s.audit_entries);
            println!("db size (bytes)  {}", s.db_size_bytes);
            ExitCode::SUCCESS
        },
        Err(e) => {
            eprintln!("admin status failed: {e}");
            ExitCode::from(1)
        },
    }
}

fn run_doctor_cmd() -> ExitCode {
    let cfg = load_or_default();
    let report = admin::doctor(&cfg);
    let mut failed = false;
    for check in &report.checks {
        let badge = match check.status {
            admin::DoctorStatus::Ok => "OK  ",
            admin::DoctorStatus::Warn => "WARN",
            admin::DoctorStatus::Fail => {
                failed = true;
                "FAIL"
            },
        };
        println!("[{badge}] {:<24} {}", check.name, check.detail);
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
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
    init                            One-time first-boot setup
    serve                           Run the server
    admin token regenerate          Rotate the admin secret + print a new token
    admin status                    Print non-sensitive operational status
    doctor                          Pre-flight: file perms, ports, data dir
    --version                       Print version
    --help                          Print this message

ENVIRONMENT:
    MEET_ADMIN_PASSPHRASE   Read instead of prompting on a TTY.
",
        version = env!("CARGO_PKG_VERSION")
    );
}
