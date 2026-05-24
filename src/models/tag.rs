use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::error::AppResult;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Tag {
    pub id: i32,
    pub name: String,
    pub grp: i16,
    pub created_at: Option<DateTime<Utc>>,
    pub icon: String,
}

impl Tag {
    pub async fn find_by_name(pool: &PgPool, name: &str) -> AppResult<Option<Self>> {
        Ok(
            sqlx::query_as::<_, Self>("SELECT * FROM tags WHERE name = $1 LIMIT 1")
                .bind(name)
                .fetch_optional(pool)
                .await?,
        )
    }
}
