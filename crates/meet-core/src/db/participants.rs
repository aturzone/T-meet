//! `participants` table. Filled in once Phase 03 needs persistent participants;
//! Phase 02 just exposes the type and basic insert/list so the schema is alive.

use sqlx::Row;

use crate::db::{Db, DbError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    pub id: String,
    pub room_id: String,
    pub display_name: String,
    pub joined_at: i64,
    pub left_at: Option<i64>,
}

/// Insert a participant row.
///
/// # Errors
/// [`DbError::Sqlx`] on insert failure.
pub async fn insert(db: &Db, p: &Participant) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO participants (id, room_id, display_name, joined_at, left_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&p.id)
    .bind(&p.room_id)
    .bind(&p.display_name)
    .bind(p.joined_at)
    .bind(p.left_at)
    .execute(&db.pool)
    .await?;
    Ok(())
}

/// List participants for a given room.
///
/// # Errors
/// [`DbError::Sqlx`] on query failure.
pub async fn list_for_room(db: &Db, room_id: &str) -> Result<Vec<Participant>, DbError> {
    let rows = sqlx::query(
        "SELECT id, room_id, display_name, joined_at, left_at
         FROM participants WHERE room_id = ? ORDER BY joined_at",
    )
    .bind(room_id)
    .fetch_all(&db.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Participant {
            id: r.get("id"),
            room_id: r.get("room_id"),
            display_name: r.get("display_name"),
            joined_at: r.get("joined_at"),
            left_at: r.get("left_at"),
        })
        .collect())
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    async fn seed_room(db: &Db, id: &str) {
        sqlx::query(
            "INSERT INTO rooms (id, name, password_hash, salt, secret_enc, created_at)
             VALUES (?, ?, '', X'', X'', 0)",
        )
        .bind(id)
        .bind(format!("r-{id}"))
        .execute(&db.pool)
        .await
        .expect("seed");
    }

    #[tokio::test]
    async fn insert_and_list() {
        let db = Db::open_in_memory().await.expect("open");
        seed_room(&db, "room-1").await;
        let p = Participant {
            id: "p1".into(),
            room_id: "room-1".into(),
            display_name: "Alice".into(),
            joined_at: 100,
            left_at: None,
        };
        insert(&db, &p).await.expect("insert");
        let got = list_for_room(&db, "room-1").await.expect("list");
        assert_eq!(got, vec![p]);
    }
}
