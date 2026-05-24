use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::error::AppResult;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Section {
    pub id: i16,
    pub name: String,
    pub kind: String,
    pub ord: i16,
    pub display: i16,
    pub hidden: bool,
    pub password: String,
    pub created_at: Option<DateTime<Utc>>,
    pub path: String,
    pub description: String,
    pub proj: i16,
    pub grp: i16,
    pub report: bool,
    /// When true, the public nav renders this section's exhibits as a bare
    /// list with no heading — used to group exhibits silently.
    pub hide_title: bool,
}

impl Section {
    pub async fn list_visible(pool: &PgPool) -> AppResult<Vec<Self>> {
        Ok(sqlx::query_as::<_, Self>(
            "SELECT * FROM sections WHERE hidden = FALSE ORDER BY ord ASC",
        )
        .fetch_all(pool)
        .await?)
    }

    pub async fn find_by_path(pool: &PgPool, path: &str) -> AppResult<Option<Self>> {
        Ok(
            sqlx::query_as::<_, Self>("SELECT * FROM sections WHERE path = $1 LIMIT 1")
                .bind(path)
                .fetch_optional(pool)
                .await?,
        )
    }
}
