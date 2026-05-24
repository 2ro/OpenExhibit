use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::error::AppResult;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Media {
    pub id: i32,
    pub ref_id: i32,
    pub obj_type: String,
    pub mime: String,
    pub tags: String,
    pub file: String,
    pub thumb: String,
    pub file_replace: String,
    pub title: String,
    pub caption: String,
    pub width: i32,
    pub height: i32,
    pub width_resp: i32,
    pub height_resp: i32,
    pub bytes: i32,
    pub updated_at: Option<DateTime<Utc>>,
    pub uploaded_at: Option<DateTime<Utc>>,
    pub ord: i16,
    pub hidden: bool,
    pub dir: String,
    pub src: String,
}

impl Media {
    pub async fn list_for_exhibit(pool: &PgPool, exhibit_id: i32) -> AppResult<Vec<Self>> {
        Ok(sqlx::query_as::<_, Self>(
            "SELECT * FROM media
             WHERE ref_id = $1 AND obj_type = 'exhibits' AND hidden = FALSE
             ORDER BY ord ASC, id ASC",
        )
        .bind(exhibit_id)
        .fetch_all(pool)
        .await?)
    }

    pub fn is_image(&self) -> bool {
        matches!(self.mime.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp")
    }

    pub fn is_video(&self) -> bool {
        self.mime == "mp4" || self.mime == "mov" || self.mime == "webm"
    }

    pub fn is_audio(&self) -> bool {
        self.mime == "mp3" || self.mime == "ogg"
    }
}

/// One row of the cross-exhibit admin media list — flat media columns plus the
/// owning exhibit (joined via `ref_id` when `obj_type = 'exhibits'`).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MediaListRow {
    pub id: i32,
    pub ref_id: i32,
    pub obj_type: String,
    pub mime: String,
    pub file: String,
    pub title: String,
    pub width: i32,
    pub height: i32,
    pub bytes: i32,
    pub uploaded_at: Option<DateTime<Utc>>,
    pub hidden: bool,
    pub ord: i16,
    pub exhibit_id: Option<i32>,
    pub exhibit_title: Option<String>,
}

impl MediaListRow {
    pub fn is_image(&self) -> bool {
        matches!(self.mime.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp")
    }
    pub fn is_video(&self) -> bool {
        matches!(self.mime.as_str(), "mp4" | "mov" | "webm")
    }
    pub fn is_audio(&self) -> bool {
        matches!(self.mime.as_str(), "mp3" | "ogg")
    }
}

impl Media {
    /// Paginated cross-exhibit media list for the admin top-level browser.
    /// `kind` ∈ {"all","image","video","audio","other"}; unknown values fall
    /// through to "all".
    pub async fn list_all_paginated(
        pool: &PgPool,
        kind: &str,
        show_hidden: bool,
        q: Option<&str>,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<MediaListRow>, i64)> {
        let offset = page.saturating_sub(1).max(0) * per_page;
        // Single set of WHERE predicates reused for the COUNT and the SELECT,
        // so totals always match the visible page.
        let filter_sql = "\
            ($1 = 'all' \
              OR ($1 = 'image' AND m.mime IN ('jpg','jpeg','png','gif','webp')) \
              OR ($1 = 'video' AND m.mime IN ('mp4','mov','webm')) \
              OR ($1 = 'audio' AND m.mime IN ('mp3','ogg')) \
              OR ($1 = 'other' AND m.mime NOT IN \
                  ('jpg','jpeg','png','gif','webp','mp4','mov','webm','mp3','ogg'))) \
            AND ($2 OR m.hidden = FALSE) \
            AND ($3::text IS NULL \
                 OR m.title ILIKE '%' || $3 || '%' \
                 OR m.file  ILIKE '%' || $3 || '%' \
                 OR COALESCE(e.title,'') ILIKE '%' || $3 || '%')";

        let select_sql = format!(
            "SELECT m.id, m.ref_id, m.obj_type, m.mime, m.file, m.title, \
                    m.width, m.height, m.bytes, m.uploaded_at, m.hidden, m.ord, \
                    e.id AS exhibit_id, e.title AS exhibit_title \
             FROM media m \
             LEFT JOIN exhibits e ON m.obj_type = 'exhibits' AND e.id = m.ref_id \
             WHERE {filter_sql} \
             ORDER BY m.uploaded_at DESC NULLS LAST, m.id DESC \
             LIMIT $4 OFFSET $5"
        );
        let count_sql = format!(
            "SELECT COUNT(*) FROM media m \
             LEFT JOIN exhibits e ON m.obj_type = 'exhibits' AND e.id = m.ref_id \
             WHERE {filter_sql}"
        );

        let rows = sqlx::query_as::<_, MediaListRow>(&select_sql)
            .bind(kind)
            .bind(show_hidden)
            .bind(q)
            .bind(per_page)
            .bind(offset)
            .fetch_all(pool)
            .await?;
        let total: (i64,) = sqlx::query_as(&count_sql)
            .bind(kind)
            .bind(show_hidden)
            .bind(q)
            .fetch_one(pool)
            .await?;
        Ok((rows, total.0))
    }
}
