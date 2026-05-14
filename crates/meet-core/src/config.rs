use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub log: LogConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub bind_ip: IpAddr,
    pub tls_port: u16,
    pub http_redirect_port: u16,
    pub external_host: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogConfig {
    pub level: String,
    pub format: LogFormat,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Pretty,
    Json,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            tls_port: 8443,
            http_redirect_port: 8080,
            external_host: None,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("data"),
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            format: LogFormat::Pretty,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path)?;
        let cfg: Config = toml::from_str(&text)?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        let cfg: Config = toml::from_str(text)?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<(), Error> {
        if self.server.tls_port == 0 {
            return Err(Error::Config("server.tls_port must be non-zero".into()));
        }
        if self.server.tls_port == self.server.http_redirect_port {
            return Err(Error::Config(
                "server.tls_port and server.http_redirect_port must differ".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        let cfg = Config::default();
        assert!(cfg.validate().is_ok());
        assert_eq!(cfg.server.tls_port, 8443);
        assert_eq!(cfg.log.format, LogFormat::Pretty);
    }

    #[test]
    fn rejects_zero_tls_port() {
        let mut cfg = Config::default();
        cfg.server.tls_port = 0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn rejects_equal_ports() {
        let mut cfg = Config::default();
        cfg.server.tls_port = 8080;
        cfg.server.http_redirect_port = 8080;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn parses_minimal_toml() {
        let toml = r#"
            [server]
            bind_ip = "127.0.0.1"
            tls_port = 9443
            http_redirect_port = 9080

            [storage]
            data_dir = "/var/lib/meet"

            [log]
            level = "debug"
            format = "json"
        "#;
        let cfg = Config::parse(toml).expect("parse");
        assert_eq!(cfg.server.tls_port, 9443);
        assert_eq!(cfg.log.format, LogFormat::Json);
    }
}
