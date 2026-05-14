//! Append-only audit log.

use sqlx::Row;

use crate::db::{Db, DbError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub at: i64,
    pub actor: String,
    pub action: String,
    pub target: Option<String>,
    pub request_id: Option<String>,
    pub details_json: String,
}

impl Entry {
    #[must_use]
    pub fn new(actor: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            at: now_unix_seconds(),
            actor: actor.into(),
            action: action.into(),
            target: None,
            request_id: None,
            details_json: "{}".into(),
        }
    }

    #[must_use]
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    #[must_use]
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

fn now_unix_seconds() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

/// Append an entry. Never fails for "no row inserted" — sqlite never returns
/// that unless the write itself errored.
///
/// # Errors
/// [`DbError::Sqlx`] on insert failure.
pub async fn append(db: &Db, e: &Entry) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO audit_log (at, actor, action, target, request_id, details_json)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(e.at)
    .bind(&e.actor)
    .bind(&e.action)
    .bind(&e.target)
    .bind(&e.request_id)
    .bind(&e.details_json)
    .execute(&db.pool)
    .await?;
    Ok(())
}

/// List the most recent `limit` entries (newest first).
///
/// # Errors
/// [`DbError::Sqlx`] on query failure.
pub async fn recent(db: &Db, limit: i64) -> Result<Vec<Entry>, DbError> {
    let rows = sqlx::query(
        "SELECT at, actor, action, target, request_id, details_json
         FROM audit_log ORDER BY id DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&db.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Entry {
            at: r.get("at"),
            actor: r.get("actor"),
            action: r.get("action"),
            target: r.get("target"),
            request_id: r.get("request_id"),
            details_json: r.get("details_json"),
        })
        .collect())
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn append_and_recent() {
        let db = Db::open_in_memory().await.expect("open");
        let e = Entry::new("admin", "room.create")
            .with_target("room-xyz")
            .with_request_id("req-001");
        append(&db, &e).await.expect("append");
        let rows = recent(&db, 10).await.expect("recent");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].action, "room.create");
        assert_eq!(rows[0].target.as_deref(), Some("room-xyz"));
    }

    #[tokio::test]
    async fn recent_orders_newest_first() {
        let db = Db::open_in_memory().await.expect("open");
        for i in 0..3 {
            let e = Entry::new("admin", format!("action.{i}"));
            append(&db, &e).await.expect("append");
        }
        let rows = recent(&db, 10).await.expect("recent");
        assert_eq!(rows[0].action, "action.2");
        assert_eq!(rows[2].action, "action.0");
    }
}
