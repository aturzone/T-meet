//! Filesystem layout under `data/`.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DataPaths {
    pub root: PathBuf,
}

impl DataPaths {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn ca_blob(&self) -> PathBuf {
        self.root.join("ca.bin")
    }

    /// Public PEM copy of the CA cert (served at `/ca.crt`).
    #[must_use]
    pub fn ca_public_pem(&self) -> PathBuf {
        self.root.join("ca.crt")
    }

    #[must_use]
    pub fn leaf_cert_pem(&self) -> PathBuf {
        self.root.join("leaf.pem")
    }

    #[must_use]
    pub fn leaf_key_pem(&self) -> PathBuf {
        self.root.join("leaf.key")
    }
}
