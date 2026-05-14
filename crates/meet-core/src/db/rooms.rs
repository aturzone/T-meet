//! `rooms` table CRUD.

use sqlx::Row;

use crate::db::{Db, DbError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Room {
    pub id: String,
    pub name: String,
    pub password_hash: String,
    pub salt: Vec<u8>,
    pub secret_enc: Vec<u8>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub creator_note: Option<String>,
}

/// Insert a new room. The caller is responsible for ID uniqueness; on
/// collision sqlite returns a unique-constraint error.
///
/// # Errors
/// [`DbError::Sqlx`] on insert failure.
pub async fn insert(db: &Db, r: &Room) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO rooms (id, name, password_hash, salt, secret_enc,
                            created_at, expires_at, creator_note)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&r.id)
    .bind(&r.name)
    .bind(&r.password_hash)
    .bind(&r.salt)
    .bind(&r.secret_enc)
    .bind(r.created_at)
    .bind(r.expires_at)
    .bind(&r.creator_note)
    .execute(&db.pool)
    .await?;
    Ok(())
}

/// Fetch a room by its opaque ID.
///
/// # Errors
/// [`DbError::Sqlx`] on query failure.
pub async fn get(db: &Db, id: &str) -> Result<Option<Room>, DbError> {
    let row = sqlx::query(
        "SELECT id, name, password_hash, salt, secret_enc,
                created_at, expires_at, creator_note
         FROM rooms WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&db.pool)
    .await?;
    Ok(row.as_ref().map(row_to_room))
}

/// List all rooms.
///
/// # Errors
/// [`DbError::Sqlx`] on query failure.
pub async fn list(db: &Db) -> Result<Vec<Room>, DbError> {
    let rows = sqlx::query(
        "SELECT id, name, password_hash, salt, secret_enc,
                created_at, expires_at, creator_note
         FROM rooms ORDER BY created_at DESC",
    )
    .fetch_all(&db.pool)
    .await?;
    Ok(rows.iter().map(row_to_room).collect())
}

/// Delete a room. Returns `true` when a row was actually removed.
///
/// # Errors
/// [`DbError::Sqlx`] on delete failure.
pub async fn delete(db: &Db, id: &str) -> Result<bool, DbError> {
    let res = sqlx::query("DELETE FROM rooms WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

fn row_to_room(row: &sqlx::sqlite::SqliteRow) -> Room {
    Room {
        id: row.get("id"),
        name: row.get("name"),
        password_hash: row.get("password_hash"),
        salt: row.get("salt"),
        secret_enc: row.get("secret_enc"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        creator_note: row.get("creator_note"),
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    fn sample(id: &str) -> Room {
        Room {
            id: id.into(),
            name: format!("Room {id}"),
            password_hash: "$argon2id$v=19$m=65536,t=3,p=1$abc$def".into(),
            salt: vec![1u8; 16],
            secret_enc: vec![2u8; 64],
            created_at: 1_700_000_000,
            expires_at: None,
            creator_note: Some("smoke".into()),
        }
    }

    #[tokio::test]
    async fn insert_and_get_round_trip() {
        let db = Db::open_in_memory().await.expect("open");
        let r = sample("abc1234567890abcdef012");
        insert(&db, &r).await.expect("insert");
        let got = get(&db, &r.id).await.expect("get").expect("present");
        assert_eq!(got, r);
    }

    #[tokio::test]
    async fn delete_returns_false_for_unknown() {
        let db = Db::open_in_memory().await.expect("open");
        assert!(!delete(&db, "nope").await.expect("delete"));
    }

    #[tokio::test]
    async fn list_orders_by_created_desc() {
        let db = Db::open_in_memory().await.expect("open");
        let mut a = sample("aaaa1111");
        a.created_at = 100;
        let mut b = sample("bbbb2222");
        b.created_at = 200;
        insert(&db, &a).await.expect("a");
        insert(&db, &b).await.expect("b");
        let rooms = list(&db).await.expect("list");
        assert_eq!(rooms[0].id, "bbbb2222");
        assert_eq!(rooms[1].id, "aaaa1111");
    }

    #[tokio::test]
    async fn delete_cascades_to_participants() {
        let db = Db::open_in_memory().await.expect("open");
        let r = sample("cascade01");
        insert(&db, &r).await.expect("insert");
        sqlx::query(
            "INSERT INTO participants (id, room_id, display_name, joined_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind("p1")
        .bind(&r.id)
        .bind("Alice")
        .bind(1_700_000_000_i64)
        .execute(&db.pool)
        .await
        .expect("p");

        assert!(delete(&db, &r.id).await.expect("delete"));

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM participants WHERE room_id = ?")
            .bind(&r.id)
            .fetch_one(&db.pool)
            .await
            .expect("count");
        assert_eq!(count.0, 0, "participants must cascade");
    }
}
