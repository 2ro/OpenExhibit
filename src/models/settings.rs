use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::PgPool;

use crate::error::AppResult;

#[allow(clippy::struct_excessive_bools)] // Mirrors the legacy schema 1:1.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Settings {
    pub id: i16,
    pub site_name: String,
    pub install_date: Option<DateTime<Utc>>,
    pub version: String,
    pub site_lang: String,
    pub time_format: String,
    pub tagging: bool,
    pub help: bool,
    pub caching: bool,
    pub hibernate: String,
    pub obj_name: String,
    pub obj_theme: String,
    pub obj_itop: String,
    pub obj_ibot: String,
    pub obj_org: bool,
    pub obj_apikey: String,
    pub site_format: String,
    pub site_offset: i16,
    pub site_vars: JsonValue,
    pub smtp_host: String,
    pub smtp_port: i32,
    pub smtp_user: String,
    pub smtp_pass: String,
    pub smtp_from: String,
    /// Site-wide CSS rendered inline in every public page <head>.
    /// Per-exhibit `custom_css` is appended after this, so an exhibit can
    /// override site-wide rules.
    pub custom_css: String,
    /// Hex color (e.g. `#0004ff`) for `--color` / `--color5`. Empty = SCSS
    /// default (black). Surfaced as an HTML5 color picker in the admin.
    pub theme_text_color: String,
    /// Hex color for `--bg`. Empty = SCSS default (white).
    pub theme_bg_color: String,
    /// 4chan-style greentext mode: lines starting with `>` render as
    /// colored quote lines instead of as Markdown blockquotes.
    pub enable_greentext: bool,
}

impl Settings {
    pub async fn load(pool: &PgPool) -> AppResult<Self> {
        Ok(
            sqlx::query_as::<_, Self>("SELECT * FROM settings WHERE id = 1 LIMIT 1")
                .fetch_one(pool)
                .await?,
        )
    }
}
