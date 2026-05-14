#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use meet_core::config::Config;
use meet_core::log;

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
        "init" | "serve" => {
            let cfg = load_or_default();
            log::init(&cfg.log);
            tracing::info!(
                command = cmd,
                version = env!("CARGO_PKG_VERSION"),
                "meet-server starting"
            );
            tracing::info!("phase-00 placeholder: real `{cmd}` logic lands in later phases");
            ExitCode::SUCCESS
        },
        other => {
            eprintln!("unknown command: {other}");
            print_help();
            ExitCode::from(2)
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

fn print_help() {
    println!(
        "meet-server {version}

USAGE:
    meet-server <COMMAND>

COMMANDS:
    init        One-time first-boot setup (phase 01)
    serve       Run the server (phase 02+)
    --version   Print version
    --help      Print this message
",
        version = env!("CARGO_PKG_VERSION")
    );
}
