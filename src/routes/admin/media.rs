use std::path::{Path, PathBuf};

use actix_multipart::form::tempfile::TempFile;
use actix_multipart::form::text::Text;
use actix_multipart::form::{MultipartForm, MultipartFormConfig};
use actix_session::Session;
use actix_web::{get, post, web, HttpResponse};
use askama::Template;
use serde::Deserialize;
use sqlx::PgPool;

use crate::auth;
use crate::config::Config;
use crate::csrf;
use crate::error::{AppError, AppResult};
use crate::flash;
use crate::images;
use crate::models::exhibit::Exhibit;
use crate::models::media::{Media, MediaListRow};

#[derive(Template)]
#[template(path = "admin/media/list.html")]
struct ListPage {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    exhibit: Exhibit,
    media: Vec<Media>,
}

#[derive(Template)]
#[template(path = "admin/media/all.html")]
struct AllListPage {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    rows: Vec<MediaListRow>,
    kind: String,
    show_hidden: bool,
    q: String,
    page: i64,
    total: i64,
    total_pages: i64,
}

#[derive(Deserialize)]
struct AllListQuery {
    kind: Option<String>,
    hidden: Option<String>,
    q: Option<String>,
    page: Option<i64>,
}

#[derive(Template)]
#[template(path = "admin/media/edit.html")]
struct EditPage {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    exhibit: Exhibit,
    media: Media,
}

#[derive(Template)]
#[template(path = "admin/media/confirm_delete.html")]
struct ConfirmDelete {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    exhibit: Exhibit,
    media: Media,
}

#[derive(MultipartForm)]
struct UploadForm {
    #[multipart(rename = "_csrf")]
    csrf: Text<String>,
    #[multipart(rename = "files")]
    files: Vec<TempFile>,
}

#[derive(Deserialize)]
struct MediaEditForm {
    #[serde(rename = "_csrf")]
    csrf: String,
    title: String,
    caption: String,
    ord: i16,
    hidden: Option<String>,
}

#[derive(Deserialize)]
struct CsrfOnly {
    #[serde(rename = "_csrf")]
    csrf: String,
}

#[get("/exhibits/{id}/media")]
async fn list_get(session: Session, pool: web::Data<PgPool>, path: web::Path<i32>) -> HttpResponse {
    let me = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let exhibit_id = path.into_inner();
    let exhibit = match load_exhibit(pool.get_ref(), exhibit_id).await {
        Ok(e) => e,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let media = match Media::list_for_exhibit(pool.get_ref(), exhibit_id).await {
        Ok(m) => m,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let token = match csrf::get_or_create(&session) {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let html = (ListPage {
        page_title: format!("Media: {}", exhibit.title),
        csrf_token: token,
        user_id: me.userid,
        flash: flash::take(&session),
        exhibit,
        media,
    })
    .render();
    match html {
        Ok(h) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(h),
        Err(e) => actix_web::ResponseError::error_response(&AppError::Template(e)),
    }
}

/// Top-level cross-exhibit media browser: lists every media row joined with
/// its owning exhibit. Filter by kind/hidden/free-text and paginate.
#[get("/media")]
async fn all_list_get(
    session: Session,
    pool: web::Data<PgPool>,
    query: web::Query<AllListQuery>,
) -> HttpResponse {
    let me = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let kind = match query.kind.as_deref().unwrap_or("all") {
        k @ ("all" | "image" | "video" | "audio" | "other") => k.to_string(),
        _ => "all".to_string(),
    };
    let show_hidden = matches!(query.hidden.as_deref(), Some("1" | "true" | "show"));
    let q_raw = query.q.as_deref().unwrap_or("").trim().to_string();
    let q_arg = if q_raw.is_empty() {
        None
    } else {
        Some(q_raw.as_str())
    };
    let page = query.page.unwrap_or(1).max(1);
    let per_page: i64 = 50;

    let (rows, total) =
        match Media::list_all_paginated(pool.get_ref(), &kind, show_hidden, q_arg, page, per_page)
            .await
        {
            Ok(r) => r,
            Err(e) => return actix_web::ResponseError::error_response(&e),
        };
    let total_pages = (total + per_page - 1) / per_page;
    let token = match csrf::get_or_create(&session) {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let html = (AllListPage {
        page_title: "Media".into(),
        csrf_token: token,
        user_id: me.userid,
        flash: flash::take(&session),
        rows,
        kind,
        show_hidden,
        q: q_raw,
        page,
        total,
        total_pages,
    })
    .render();
    match html {
        Ok(h) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(h),
        Err(e) => actix_web::ResponseError::error_response(&AppError::Template(e)),
    }
}

#[post("/exhibits/{id}/media")]
async fn upload_post(
    session: Session,
    pool: web::Data<PgPool>,
    cfg: web::Data<Config>,
    path: web::Path<i32>,
    MultipartForm(form): MultipartForm<UploadForm>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    let exhibit_id = path.into_inner();
    let exhibit = match load_exhibit(pool.get_ref(), exhibit_id).await {
        Ok(e) => e,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };

    let dest_dir = PathBuf::from(&cfg.files_dir)
        .join("gimgs")
        .join(exhibit_id.to_string());
    if let Err(e) = std::fs::create_dir_all(&dest_dir) {
        return actix_web::ResponseError::error_response(&AppError::Io(e));
    }

    let total = form.files.len();
    let mut errors = Vec::new();
    let mut duplicates = 0usize;
    for tf in form.files {
        match save_one(&dest_dir, tf, pool.get_ref(), &exhibit).await {
            Ok(SaveOutcome::Stored) => {}
            Ok(SaveOutcome::Duplicate) => duplicates += 1,
            Err(e) => errors.push(e.to_string()),
        }
    }

    let failed = errors.len();
    let ok = total - failed - duplicates;
    if failed > 0 {
        tracing::warn!(upload_errors = ?errors, "some files failed to upload");
    }
    let dupe_note = if duplicates > 0 {
        format!(
            " ({duplicates} exact duplicate{} skipped — same content already in this exhibit)",
            if duplicates == 1 { "" } else { "s" }
        )
    } else {
        String::new()
    };
    let msg = if failed == 0 && ok == 0 && duplicates > 0 {
        format!(
            "Nothing new to add — {duplicates} duplicate{} skipped.",
            if duplicates == 1 { "" } else { "s" }
        )
    } else if failed == 0 {
        format!(
            "{ok} file{} uploaded{dupe_note}",
            if ok == 1 { "" } else { "s" }
        )
    } else if ok == 0 && duplicates == 0 {
        format!("Upload failed: {}", errors.join("; "))
    } else {
        format!(
            "{ok}/{total} uploaded{dupe_note}; {failed} failed: {}",
            errors.join("; ")
        )
    };
    flash::set(&session, msg);
    HttpResponse::Found()
        .append_header(("Location", format!("/admin/exhibits/{exhibit_id}/media")))
        .finish()
}

/// Result of `save_one`. Distinguishes a stored file from a duplicate
/// that was intentionally skipped, so the upload handler can report
/// counts to the user.
enum SaveOutcome {
    Stored,
    Duplicate,
}

async fn save_one(
    dest_dir: &Path,
    tf: TempFile,
    pool: &PgPool,
    exhibit: &Exhibit,
) -> AppResult<SaveOutcome> {
    use sha2::{Digest, Sha256};

    let original_name = tf.file_name.as_deref().unwrap_or("upload");
    let safe_name = sanitize_filename(original_name);
    if safe_name.is_empty() {
        return Err(AppError::BadRequest("invalid filename".into()));
    }
    let size = tf.size;
    if size > MAX_UPLOAD_PER_FILE {
        return Err(AppError::BadRequest(format!(
            "file exceeds per-file limit of {MAX_UPLOAD_PER_FILE} bytes"
        )));
    }
    let temp_path = tf.file.path().to_path_buf();

    // Magic-byte detect by reading first 16 bytes.
    let bytes = std::fs::read(&temp_path)?;
    let mime =
        detect_mime(&bytes).ok_or_else(|| AppError::BadRequest("unsupported file type".into()))?;

    // Hash the raw uploaded bytes. We do this before re-encoding so
    // the dedup key matches what the user uploaded, not what we
    // round-tripped through the image crate.
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let sha = format!("{:x}", hasher.finalize());

    // Same hash already attached to this exhibit? Skip everything —
    // don't write the file, don't insert a row. Lets the user re-drop
    // a folder of mixed old + new files and only the new ones land.
    let existing: Option<(i32,)> = sqlx::query_as(
        "SELECT id FROM media
         WHERE ref_id = $1 AND obj_type = 'exhibits' AND sha256 = $2
         LIMIT 1",
    )
    .bind(exhibit.id)
    .bind(&sha)
    .fetch_optional(pool)
    .await?;
    if existing.is_some() {
        return Ok(SaveOutcome::Duplicate);
    }

    let (width, height, final_bytes) = if is_image_mime(mime) {
        // Load through image crate, apply EXIF orientation, re-encode.
        let img = images::load_oriented(&temp_path)?;
        let (w, h) = (img.width(), img.height());
        let mut buf = std::io::Cursor::new(Vec::new());
        let fmt = match mime {
            "png" => image::ImageFormat::Png,
            "gif" => image::ImageFormat::Gif,
            "webp" => image::ImageFormat::WebP,
            _ => image::ImageFormat::Jpeg,
        };
        img.write_to(&mut buf, fmt)?;
        (
            i32::try_from(w).unwrap_or(0),
            i32::try_from(h).unwrap_or(0),
            buf.into_inner(),
        )
    } else {
        // Non-image (mp4 etc.) — store as-is, no dimensions.
        (0, 0, bytes)
    };

    // Resolve collisions deterministically: if `safe_name` already exists,
    // append a short random suffix so we never silently overwrite.
    let safe_name = unique_filename_in(dest_dir, &safe_name);
    let dest_path = dest_dir.join(&safe_name);
    std::fs::write(&dest_path, &final_bytes)?;

    sqlx::query(
        "INSERT INTO media (ref_id, obj_type, mime, file, title, width, height,
                            uploaded_at, ord, bytes, sha256)
         VALUES ($1, 'exhibits', $2, $3, $4, $5, $6, now(),
                 COALESCE((SELECT MAX(ord)+1 FROM media WHERE ref_id = $1 AND obj_type = 'exhibits'), 1),
                 $7, $8)",
    )
    .bind(exhibit.id)
    .bind(mime)
    .bind(&safe_name)
    .bind(&safe_name)
    .bind(width)
    .bind(height)
    .bind(i32::try_from(size).unwrap_or(0))
    .bind(&sha)
    .execute(pool)
    .await?;
    Ok(SaveOutcome::Stored)
}

fn sanitize_filename(name: &str) -> String {
    let stem = Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    stem.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('.')
        .to_string()
}

/// Return a name that does not collide with anything in `dir`. If `name` is
/// free, return it as-is; otherwise insert a short random token before the
/// extension. Prevents the (admin-side) footgun where two uploads with names
/// that sanitize to the same string would silently overwrite each other.
fn unique_filename_in(dir: &Path, name: &str) -> String {
    use rand::Rng;
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    for _ in 0..16 {
        let suffix: u32 = rand::thread_rng().gen();
        let candidate = format!("{stem}-{suffix:08x}{ext}");
        if !dir.join(&candidate).exists() {
            return candidate;
        }
    }
    // Astronomically unlikely; fall back to a timestamp.
    format!(
        "{stem}-{}{ext}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    )
}

fn detect_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() < 4 {
        return None;
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Some("png");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        return Some("mp4");
    }
    None
}

fn is_image_mime(mime: &str) -> bool {
    matches!(mime, "jpg" | "jpeg" | "png" | "gif" | "webp")
}

/// True iff `entry` is a derivative-image filename for `original` —
/// i.e. matches the layout `{shape}_{size}_{original}` written by
/// `images::derivative_path`.
fn is_derivative_of(entry: &str, original: &str) -> bool {
    let Some((shape, rest)) = entry.split_once('_') else {
        return false;
    };
    let Some((size_str, name)) = rest.split_once('_') else {
        return false;
    };
    if name == original && size_str.parse::<u32>().is_ok() && images::Shape::parse(shape).is_some()
    {
        return true;
    }
    // Two-token shape names ("four_three", "three_two").
    if let Some((shape2, rest2)) = rest.split_once('_') {
        let combined = format!("{shape}_{size_str}");
        if let Some((real_size, real_name)) = rest2.split_once('_') {
            if real_name == original
                && real_size.parse::<u32>().is_ok()
                && images::Shape::parse(&combined).is_some()
            {
                return true;
            }
        }
        let _ = shape2;
    }
    false
}

#[get("/exhibits/{eid}/media/{mid}/edit")]
async fn edit_get(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<(i32, i32)>,
) -> HttpResponse {
    let me = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let (eid, mid) = path.into_inner();
    let exhibit = match load_exhibit(pool.get_ref(), eid).await {
        Ok(e) => e,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let media = match load_media(pool.get_ref(), mid, eid).await {
        Ok(m) => m,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let token = match csrf::get_or_create(&session) {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let html = (EditPage {
        page_title: format!("Edit: {}", media.file),
        csrf_token: token,
        user_id: me.userid,
        flash: flash::take(&session),
        exhibit,
        media,
    })
    .render();
    match html {
        Ok(h) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(h),
        Err(e) => actix_web::ResponseError::error_response(&AppError::Template(e)),
    }
}

#[post("/exhibits/{eid}/media/{mid}/edit")]
async fn edit_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<(i32, i32)>,
    form: web::Form<MediaEditForm>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    let (eid, mid) = path.into_inner();
    if let Err(e) = sqlx::query(
        "UPDATE media SET title = $1, caption = $2, ord = $3, hidden = $4, updated_at = now()
         WHERE id = $5 AND ref_id = $6",
    )
    .bind(&form.title)
    .bind(&form.caption)
    .bind(form.ord)
    .bind(form.hidden.is_some())
    .bind(mid)
    .bind(eid)
    .execute(pool.get_ref())
    .await
    {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    flash::set(&session, "Media saved");
    HttpResponse::Found()
        .append_header(("Location", format!("/admin/exhibits/{eid}/media")))
        .finish()
}

#[get("/exhibits/{eid}/media/{mid}/confirm-delete")]
async fn confirm_delete_get(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<(i32, i32)>,
) -> HttpResponse {
    let me = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let (eid, mid) = path.into_inner();
    let exhibit = match load_exhibit(pool.get_ref(), eid).await {
        Ok(e) => e,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let media = match load_media(pool.get_ref(), mid, eid).await {
        Ok(m) => m,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let token = match csrf::get_or_create(&session) {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let html = (ConfirmDelete {
        page_title: format!("Delete: {}", media.file),
        csrf_token: token,
        user_id: me.userid,
        flash: flash::take(&session),
        exhibit,
        media,
    })
    .render();
    match html {
        Ok(h) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(h),
        Err(e) => actix_web::ResponseError::error_response(&AppError::Template(e)),
    }
}

#[post("/exhibits/{eid}/media/{mid}/delete")]
async fn delete_post(
    session: Session,
    pool: web::Data<PgPool>,
    cfg: web::Data<Config>,
    path: web::Path<(i32, i32)>,
    form: web::Form<CsrfOnly>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    let (eid, mid) = path.into_inner();
    let media = match load_media(pool.get_ref(), mid, eid).await {
        Ok(m) => m,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };

    let gimg = PathBuf::from(&cfg.files_dir)
        .join("gimgs")
        .join(eid.to_string())
        .join(&media.file);
    let _ = std::fs::remove_file(&gimg);

    // Drop all derivatives that match the precise pattern {shape}_{size}_{file}.
    // (Naïve ends_with would also match e.g. "another_logo.jpg" → "logo.jpg".)
    let dimgs = PathBuf::from(&cfg.files_dir)
        .join("dimgs")
        .join(eid.to_string());
    if let Ok(rd) = std::fs::read_dir(&dimgs) {
        for entry in rd.flatten() {
            if is_derivative_of(&entry.file_name().to_string_lossy(), &media.file) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    if let Err(e) = sqlx::query("DELETE FROM media WHERE id = $1 AND ref_id = $2")
        .bind(mid)
        .bind(eid)
        .execute(pool.get_ref())
        .await
    {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    flash::set(&session, "Media deleted");
    HttpResponse::Found()
        .append_header(("Location", format!("/admin/exhibits/{eid}/media")))
        .finish()
}

/// Bulk reorder via per-row `ord_{mid}` number inputs on the media
/// list page. Mirrors `exhibits::reorder_post`. Same handler used for
/// the "Save order" button at the bottom of the redesigned card grid.
#[post("/exhibits/{eid}/media/reorder")]
async fn reorder_bulk_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    let token = form.get("_csrf").cloned().unwrap_or_default();
    if !csrf::verify(&session, &token) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    let eid = path.into_inner();
    let pool_ref = pool.get_ref();
    let mut tx = match pool_ref.begin().await {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&AppError::Db(e)),
    };
    for (k, v) in form.iter() {
        let Some(id_str) = k.strip_prefix("ord_") else {
            continue;
        };
        let (Ok(id), Ok(ord)) = (id_str.parse::<i32>(), v.parse::<i16>()) else {
            continue;
        };
        // Constrain to this exhibit so a stray ord_X for someone else's
        // media row can't get bumped from this form.
        if let Err(e) = sqlx::query(
            "UPDATE media SET ord = $1 WHERE id = $2 AND ref_id = $3 AND obj_type = 'exhibits'",
        )
        .bind(ord)
        .bind(id)
        .bind(eid)
        .execute(&mut *tx)
        .await
        {
            return actix_web::ResponseError::error_response(&AppError::Db(e));
        }
    }
    if let Err(e) = tx.commit().await {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    flash::set(&session, "Order saved");
    HttpResponse::Found()
        .append_header(("Location", format!("/admin/exhibits/{eid}/media")))
        .finish()
}

#[post("/exhibits/{eid}/media/{mid}/move-up")]
async fn move_up_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<(i32, i32)>,
    form: web::Form<CsrfOnly>,
) -> HttpResponse {
    reorder(session, pool, path, form, true).await
}

#[post("/exhibits/{eid}/media/{mid}/move-down")]
async fn move_down_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<(i32, i32)>,
    form: web::Form<CsrfOnly>,
) -> HttpResponse {
    reorder(session, pool, path, form, false).await
}

async fn reorder(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<(i32, i32)>,
    form: web::Form<CsrfOnly>,
    up: bool,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    let (eid, mid) = path.into_inner();
    let _ = swap_with_neighbor(pool.get_ref(), eid, mid, up).await;
    HttpResponse::Found()
        .append_header(("Location", format!("/admin/exhibits/{eid}/media")))
        .finish()
}

async fn swap_with_neighbor(pool: &PgPool, eid: i32, mid: i32, up: bool) -> AppResult<()> {
    let media = load_media(pool, mid, eid).await?;
    let sql = if up {
        "SELECT * FROM media WHERE ref_id = $1 AND obj_type = 'exhibits' AND ord < $2 ORDER BY ord DESC LIMIT 1"
    } else {
        "SELECT * FROM media WHERE ref_id = $1 AND obj_type = 'exhibits' AND ord > $2 ORDER BY ord ASC LIMIT 1"
    };
    let neighbor: Option<Media> = sqlx::query_as(sql)
        .bind(eid)
        .bind(media.ord)
        .fetch_optional(pool)
        .await?;
    let Some(n) = neighbor else { return Ok(()) };
    let (a, b) = if media.ord == n.ord {
        if up {
            (media.ord - 1, media.ord)
        } else {
            (media.ord + 1, media.ord)
        }
    } else {
        (n.ord, media.ord)
    };
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE media SET ord = $1 WHERE id = $2")
        .bind(a)
        .bind(media.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE media SET ord = $1 WHERE id = $2")
        .bind(b)
        .bind(n.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn load_exhibit(pool: &PgPool, id: i32) -> AppResult<Exhibit> {
    sqlx::query_as::<_, Exhibit>("SELECT * FROM exhibits WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

async fn load_media(pool: &PgPool, mid: i32, eid: i32) -> AppResult<Media> {
    sqlx::query_as::<_, Media>(
        "SELECT * FROM media WHERE id = $1 AND ref_id = $2 AND obj_type = 'exhibits'",
    )
    .bind(mid)
    .bind(eid)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

// Lazy thumbnail/derivative endpoint. Pattern: {shape}_{size}_{original_name}.
#[get("/files/dimgs/{ref_id}/{filename}")]
async fn derivative_get(cfg: web::Data<Config>, path: web::Path<(i32, String)>) -> HttpResponse {
    let (ref_id, filename) = path.into_inner();
    // Path traversal protection.
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return HttpResponse::BadRequest().body("invalid filename");
    }
    let Some((shape_str, rest)) = filename.split_once('_') else {
        return HttpResponse::BadRequest().body("expected {shape}_{size}_{name}");
    };
    let Some((size_str, original_name)) = rest.split_once('_') else {
        return HttpResponse::BadRequest().body("expected {shape}_{size}_{name}");
    };
    // Multi-word shape names (four_three, three_two, over_and_over) — try a couple compositions.
    let (shape, size, original_name) = if let Some(s) = images::Shape::parse(shape_str) {
        let size: u32 = match size_str.parse() {
            Ok(s) if (16..=4096).contains(&s) => s,
            _ => return HttpResponse::BadRequest().body("invalid size"),
        };
        (s, size, original_name.to_string())
    } else {
        // shape_str is the first token, but shape may be e.g. "four_three" — recombine.
        let combined_two = format!("{shape_str}_{size_str}");
        let Some((real_size_str, real_name)) = original_name.split_once('_') else {
            return HttpResponse::BadRequest().body("invalid format");
        };
        match images::Shape::parse(&combined_two) {
            Some(s) => {
                let size: u32 = match real_size_str.parse() {
                    Ok(s) if (16..=4096).contains(&s) => s,
                    _ => return HttpResponse::BadRequest().body("invalid size"),
                };
                (s, size, real_name.to_string())
            }
            None => return HttpResponse::BadRequest().body("unknown shape"),
        }
    };

    // original_name must also be path-safe.
    if original_name.contains('/') || original_name.contains('\\') || original_name.contains("..") {
        return HttpResponse::BadRequest().body("invalid filename");
    }

    let files_dir = PathBuf::from(&cfg.files_dir);
    let derivative =
        match images::ensure_derivative(&files_dir, ref_id, shape, size, &original_name) {
            Ok(p) => p,
            Err(AppError::NotFound) => return HttpResponse::NotFound().finish(),
            Err(e) => return actix_web::ResponseError::error_response(&e),
        };
    match std::fs::read(&derivative) {
        Ok(bytes) => HttpResponse::Ok()
            .content_type(content_type_for(&original_name))
            .append_header(("Cache-Control", "public, max-age=31536000, immutable"))
            .body(bytes),
        Err(_) => HttpResponse::NotFound().finish(),
    }
}

fn content_type_for(filename: &str) -> &'static str {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/jpeg",
    }
}

/// Per-file 50 MiB, total 250 MiB. Big enough for high-res photos / short
/// video, small enough to avoid trivial disk-exhaustion attacks. The total
/// limit is enforced by `MultipartFormConfig`; the per-file limit is checked
/// in `save_one` (actix-multipart 0.7 has no per-field size cap on `TempFile`).
const MAX_UPLOAD_PER_FILE: usize = 50 * 1024 * 1024;
const MAX_UPLOAD_TOTAL: usize = 250 * 1024 * 1024;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.app_data(
        MultipartFormConfig::default()
            .total_limit(MAX_UPLOAD_TOTAL)
            .memory_limit(2 * 1024 * 1024),
    )
    .service(all_list_get)
    .service(list_get)
    .service(upload_post)
    .service(edit_get)
    .service(edit_post)
    .service(confirm_delete_get)
    .service(delete_post)
    .service(reorder_bulk_post)
    .service(move_up_post)
    .service(move_down_post);
}

pub fn configure_public(cfg: &mut web::ServiceConfig) {
    cfg.service(derivative_get);
}
