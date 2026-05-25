use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::PgPool;

use crate::error::AppResult;

#[allow(clippy::struct_excessive_bools)] // Mirrors the legacy schema 1:1.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Exhibit {
    pub id: i32,
    pub kind: String,
    pub ref_id: i32,
    pub title: String,
    pub content: String,
    pub is_home: bool,
    pub link: String,
    pub link_target: bool,
    pub iframe: bool,
    pub is_new: bool,
    pub tags: String,
    pub header: String,
    pub updated_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub creator: i16,
    pub status: i16,
    pub process: bool,
    pub page_cache: bool,
    pub section_id: i16,
    pub section_top: bool,
    pub section_sub: String,
    pub subdir: bool,
    pub url: String,
    pub ord: i16,
    pub color: String,
    pub bgimg: String,
    pub hidden: bool,
    pub current_flag: bool,
    pub perm: bool,
    pub media_source: i16,
    pub media_source_detail: String,
    pub images: i16,
    pub thumbs_shape: i16,
    pub thumbs: i16,
    pub format: String,
    pub thumbs_format: i16,
    pub operand: i16,
    pub titling: i16,
    pub break_count: i16,
    pub tiling: bool,
    pub year: String,
    pub report: bool,
    pub password: String,
    pub placement: i16,
    pub template: String,
    pub extra: JsonValue,
    /// Per-exhibit CSS rendered inline in the page <head>. Empty by default.
    pub custom_css: String,
    /// Per-exhibit override of `--color` / `--bg`. Empty falls back to
    /// the site-wide pickers in settings, then to the SCSS defaults.
    pub theme_text_color: String,
    pub theme_bg_color: String,
}

impl Exhibit {
    pub async fn find_home(pool: &PgPool) -> AppResult<Option<Self>> {
        Ok(sqlx::query_as::<_, Self>(
            "SELECT * FROM exhibits WHERE is_home = TRUE AND status = 1 LIMIT 1",
        )
        .fetch_optional(pool)
        .await?)
    }

    pub async fn find_by_url(pool: &PgPool, url: &str) -> AppResult<Option<Self>> {
        Ok(sqlx::query_as::<_, Self>(
            "SELECT * FROM exhibits WHERE url = $1 AND status = 1 LIMIT 1",
        )
        .bind(url)
        .fetch_optional(pool)
        .await?)
    }

    pub async fn list_for_section(pool: &PgPool, section_id: i16) -> AppResult<Vec<Self>> {
        Ok(sqlx::query_as::<_, Self>(
            "SELECT * FROM exhibits
             WHERE section_id = $1 AND kind = 'exhibits' AND status = 1 AND hidden = FALSE
             ORDER BY ord ASC, id ASC",
        )
        .bind(section_id)
        .fetch_all(pool)
        .await?)
    }

    pub async fn list_top_of_each_section(pool: &PgPool) -> AppResult<Vec<Self>> {
        Ok(sqlx::query_as::<_, Self>(
            "SELECT * FROM exhibits
             WHERE section_top = TRUE AND status = 1 AND hidden = FALSE
             ORDER BY section_id ASC, ord ASC",
        )
        .fetch_all(pool)
        .await?)
    }
}
