use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::error::AppResult;

#[allow(clippy::struct_excessive_bools, clippy::struct_field_names)] // Mirrors the legacy schema.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: i32,
    pub userid: String,
    pub password_hash: String,
    pub email: String,
    pub threads: i16,
    pub writing: bool,
    pub offset_: i16,
    pub date_format: String,
    pub lang: String,
    pub user_hash: String,
    pub help: bool,
    pub mode: i16,
    pub first_name: String,
    pub last_name: String,
    pub is_admin: bool,
    pub is_active: bool,
    pub is_client: bool,
    pub img: String,
    pub reset_token: Option<String>,
    pub reset_expires: Option<DateTime<Utc>>,
}

impl User {
    pub async fn find_by_userid(pool: &PgPool, userid: &str) -> AppResult<Option<Self>> {
        Ok(sqlx::query_as::<_, Self>(
            "SELECT * FROM users WHERE userid = $1 AND is_active = TRUE LIMIT 1",
        )
        .bind(userid)
        .fetch_optional(pool)
        .await?)
    }

    pub async fn find_by_id(pool: &PgPool, id: i32) -> AppResult<Option<Self>> {
        Ok(
            sqlx::query_as::<_, Self>("SELECT * FROM users WHERE id = $1 LIMIT 1")
                .bind(id)
                .fetch_optional(pool)
                .await?,
        )
    }

    pub async fn admin_exists(pool: &PgPool) -> AppResult<bool> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_admin = TRUE")
            .fetch_one(pool)
            .await?;
        Ok(row.0 > 0)
    }

    pub async fn insert(
        pool: &PgPool,
        userid: &str,
        password_hash: &str,
        email: &str,
        first_name: &str,
        last_name: &str,
        is_admin: bool,
    ) -> AppResult<i32> {
        let row: (i32,) = sqlx::query_as(
            "INSERT INTO users (userid, password_hash, email, first_name, last_name, is_admin)
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
        )
        .bind(userid)
        .bind(password_hash)
        .bind(email)
        .bind(first_name)
        .bind(last_name)
        .bind(is_admin)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }
}
