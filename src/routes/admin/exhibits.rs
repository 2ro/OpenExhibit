use std::collections::HashMap;

use actix_session::Session;
use actix_web::{get, post, web, HttpResponse};
use askama::Template;
use serde::Deserialize;
use sqlx::PgPool;

use crate::auth;
use crate::csrf;
use crate::error::{AppError, AppResult};
use crate::flash;
use crate::formats::{self, FormatCapabilities};
use crate::models::exhibit::Exhibit;
use crate::models::section::Section;

/// One row on the redesigned exhibits list. Carries the format's
/// display name + a first-media thumbnail URL so the template doesn't
/// have to do its own lookup per row.
pub struct ExhibitRow {
    pub id: i32,
    pub title: String,
    pub url: String,
    pub format_key: String,
    pub format_display: String,
    pub status: i16, // 0 = draft, 1 = published
    pub ord: i16,
    pub section_top: bool,
    pub hidden: bool,
    pub thumb_url: Option<String>,
    /// Set when the format's `uses_media` capability is true. Drives
    /// the "missing thumbnail" placeholder when `thumb_url` is None
    /// (i.e. this exhibit should have media but doesn't yet).
    pub format_uses_media: bool,
}

/// One section in the grouped list — section name + display label +
/// the exhibits filed under it.
pub struct ExhibitSectionGroup {
    pub id: i16,
    pub name: String,
    pub hidden: bool,
    pub exhibits: Vec<ExhibitRow>,
}

#[derive(Template)]
#[template(path = "admin/exhibits/list.html")]
struct ListPage {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    groups: Vec<ExhibitSectionGroup>,
    /// Exhibits whose `section_id` doesn't match any section row (orphans
    /// from a deleted section). Rendered separately at the bottom.
    orphans: Vec<ExhibitRow>,
    total: usize,
}

pub struct SectionOption {
    pub id: i16,
    pub description: String,
    pub is_current: bool,
}

pub struct FormatOption {
    pub key: String,
    pub display_name: String,
    pub description: String,
    pub selected: bool,
}

#[derive(Template)]
#[template(path = "admin/exhibits/edit.html")]
struct EditPage {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    exhibit: Exhibit,
    sections: Vec<SectionOption>,
    formats: Vec<FormatOption>,
    tag_names: String,
    /// Resolved capabilities for the exhibit's *current* format, so the
    /// template hides fields the type doesn't use (e.g. media uploader
    /// for an external-link exhibit).
    caps: FormatCapabilities,
    format_display: String,
}

#[derive(Template)]
#[template(path = "admin/exhibits/pick_format.html")]
struct PickFormatPage {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    formats: Vec<FormatOption>,
}

#[derive(Deserialize)]
struct PickFormatForm {
    #[serde(rename = "_csrf")]
    csrf: String,
    format: String,
}

#[derive(Template)]
#[template(path = "admin/exhibits/confirm_delete.html")]
struct ConfirmDelete {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    exhibit: Exhibit,
}

#[derive(Deserialize)]
struct ExhibitForm {
    #[serde(rename = "_csrf")]
    csrf: String,
    title: String,
    url: String,
    section_id: i16,
    format: String,
    // Capability-gated fields default when absent so a format that hides
    // them (e.g. external_link hiding `thumbs` and `content`) still POSTs
    // successfully. The edit template also renders hidden <input>s
    // preserving the current DB value, so round-tripping doesn't drop data.
    #[serde(default = "default_thumbs")]
    thumbs: i16,
    status: i16,
    #[serde(default)]
    content: String,
    is_home: Option<String>,
    hidden: Option<String>,
    section_top: Option<String>,
    is_new: Option<String>,
    #[serde(default)]
    password: String,
    #[serde(default)]
    tag_names: String,
    #[serde(default)]
    link: String,
    link_target: Option<String>,
    #[serde(default)]
    custom_css: String,
    /// Per-exhibit color picker — same format as the site-wide knobs
    /// in /admin/settings. Empty → inherit from settings → SCSS default.
    #[serde(default)]
    theme_text_color: String,
    #[serde(default)]
    theme_bg_color: String,
    /// Routes the post-save redirect: `media` → /admin/exhibits/{id}/media,
    /// anything else → /admin/exhibits. Set by the "Save & add media"
    /// button via `<button name="next" value="media">`.
    #[serde(default)]
    next: String,
}

fn default_thumbs() -> i16 {
    200
}

#[derive(Deserialize)]
struct CsrfOnly {
    #[serde(rename = "_csrf")]
    csrf: String,
}

#[get("/exhibits")]
async fn list_get(session: Session, pool: web::Data<PgPool>) -> HttpResponse {
    let user = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    match list_inner(&session, pool.get_ref(), &user.userid).await {
        Ok(r) => r,
        Err(e) => actix_web::ResponseError::error_response(&e),
    }
}

async fn list_inner(session: &Session, pool: &PgPool, userid: &str) -> AppResult<HttpResponse> {
    // One query, one round-trip: join each exhibit with its first
    // (lowest-ord) media row, and with its section. Rows where
    // `section_name` is NULL get bucketed as orphans below.
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i32,
        title: String,
        url: String,
        format: String,
        status: i16,
        ord: i16,
        section_top: bool,
        hidden: bool,
        section_id: Option<i16>,
        section_name: Option<String>,
        section_hidden: Option<bool>,
        first_media_file: Option<String>,
        first_media_ref_id: Option<i32>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT e.id, e.title, e.url, e.format, e.status, e.ord,
                e.section_top, e.hidden,
                s.id     AS section_id,
                s.name   AS section_name,
                s.hidden AS section_hidden,
                fm.file  AS first_media_file,
                fm.ref_id AS first_media_ref_id
         FROM exhibits e
         LEFT JOIN sections s ON s.id = e.section_id
         LEFT JOIN LATERAL (
             SELECT m.file, m.ref_id FROM media m
             WHERE m.ref_id = e.id AND m.obj_type = 'exhibits' AND m.hidden = FALSE
             ORDER BY m.ord ASC, m.id ASC LIMIT 1
         ) fm ON TRUE
         WHERE e.kind = 'exhibits'
         ORDER BY COALESCE(s.ord, 999), e.section_id, e.ord, e.id",
    )
    .fetch_all(pool)
    .await?;

    let mut groups: Vec<ExhibitSectionGroup> = Vec::new();
    let mut orphans: Vec<ExhibitRow> = Vec::new();
    let total = rows.len();

    for r in rows {
        let format = formats::find(&r.format);
        let caps = format.capabilities();
        let row = ExhibitRow {
            id: r.id,
            title: if r.title.is_empty() {
                "(untitled)".into()
            } else {
                r.title
            },
            url: r.url,
            format_key: format.key().to_string(),
            format_display: format.display_name().to_string(),
            status: r.status,
            ord: r.ord,
            section_top: r.section_top,
            hidden: r.hidden,
            thumb_url: match (r.first_media_file, r.first_media_ref_id) {
                (Some(file), Some(ref_id)) => {
                    Some(format!("/files/dimgs/{ref_id}/proportional_120_{file}"))
                }
                _ => None,
            },
            format_uses_media: caps.uses_media,
        };

        match (r.section_id, r.section_name) {
            (Some(sid), Some(name)) => {
                if let Some(g) = groups.iter_mut().find(|g| g.id == sid) {
                    g.exhibits.push(row);
                } else {
                    groups.push(ExhibitSectionGroup {
                        id: sid,
                        name,
                        hidden: r.section_hidden.unwrap_or(false),
                        exhibits: vec![row],
                    });
                }
            }
            _ => orphans.push(row),
        }
    }

    let html = ListPage {
        page_title: "Exhibits".into(),
        csrf_token: csrf::get_or_create(session)?,
        user_id: userid.into(),
        flash: flash::take(session),
        groups,
        orphans,
        total,
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

/// Step 1 of "new exhibit": pick a format. The available formats are
/// pulled from the `crate::formats` registry, so adding a new format is
/// a single-line change in `src/formats/mod.rs` — this picker updates
/// automatically with its display name + description.
#[get("/exhibits/new")]
async fn new_get(session: Session, pool: web::Data<PgPool>) -> HttpResponse {
    let user = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let token = match csrf::get_or_create(&session) {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let html = (PickFormatPage {
        page_title: "New exhibit — pick a format".into(),
        csrf_token: token,
        user_id: user.userid,
        flash: flash::take(&session),
        formats: format_options(""),
    })
    .render();
    match html {
        Ok(h) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(h),
        Err(e) => actix_web::ResponseError::error_response(&AppError::Template(e)),
    }
}

async fn load_section_options(pool: &PgPool, current: i16) -> AppResult<Vec<SectionOption>> {
    let raw = sqlx::query_as::<_, Section>("SELECT * FROM sections ORDER BY ord, id")
        .fetch_all(pool)
        .await?;
    Ok(raw
        .into_iter()
        .map(|s| SectionOption {
            id: s.id,
            description: if s.description.is_empty() {
                s.name.clone()
            } else {
                s.description.clone()
            },
            is_current: s.id == current,
        })
        .collect())
}

/// Registry-driven format picker options. Empty `current` (used in the
/// new-flow picker) means nothing is preselected.
fn format_options(current: &str) -> Vec<FormatOption> {
    formats::registry()
        .iter()
        .map(|f| FormatOption {
            key: f.key().to_string(),
            display_name: f.display_name().to_string(),
            description: f.description().to_string(),
            selected: f.key() == current,
        })
        .collect()
}

/// Step 2 of "new exhibit": create a stub with the chosen format and
/// redirect to the edit form. The edit form's capabilities-driven
/// fieldsets then show exactly the inputs that format uses.
#[post("/exhibits/new")]
async fn new_post(
    session: Session,
    pool: web::Data<PgPool>,
    form: web::Form<PickFormatForm>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    // Reject unknown format keys outright so we never write garbage to the DB.
    if !formats::registry().iter().any(|f| f.key() == form.format) {
        return HttpResponse::BadRequest().body("unknown exhibit format");
    }
    match insert_stub(pool.get_ref(), &form.format).await {
        Ok(id) => flash::redirect(
            &session,
            "Exhibit created — fill in details below",
            format!("/admin/exhibits/{id}/edit"),
        ),
        Err(e) => actix_web::ResponseError::error_response(&e),
    }
}

/// Minimal stub row used as the landing point for step 2. The user fills
/// everything else in on the edit form.
///
/// URL slug is format-aware: formats that don't expose a slug to the admin
/// (e.g. `external_link`) get an auto-generated `/<key>-<random>/` so the
/// row is well-formed without ever asking the user about it.
async fn insert_stub(pool: &PgPool, format: &str) -> AppResult<i32> {
    let url = if formats::find(format).capabilities().requires_url_slug {
        "/untitled/".to_string()
    } else {
        let suffix: u32 = rand::random();
        format!("/{format}-{suffix:08x}/")
    };
    let row: (i32,) = sqlx::query_as(
        "INSERT INTO exhibits (title, url, section_id, format, kind, status, ord)
         VALUES ('Untitled', $2, 1, $1, 'exhibits', 0,
                 COALESCE((SELECT MAX(ord)+10 FROM exhibits WHERE section_id = 1), 10))
         RETURNING id",
    )
    .bind(format)
    .bind(&url)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

#[get("/exhibits/{id}/edit")]
async fn edit_get(session: Session, pool: web::Data<PgPool>, path: web::Path<i32>) -> HttpResponse {
    let user = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    match edit_inner(&session, pool.get_ref(), &user.userid, path.into_inner()).await {
        Ok(r) => r,
        Err(e) => actix_web::ResponseError::error_response(&e),
    }
}

async fn edit_inner(
    session: &Session,
    pool: &PgPool,
    userid: &str,
    id: i32,
) -> AppResult<HttpResponse> {
    let exhibit = sqlx::query_as::<_, Exhibit>("SELECT * FROM exhibits WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let sections = load_section_options(pool, exhibit.section_id).await?;
    let format = formats::find(&exhibit.format);
    let formats = format_options(&exhibit.format);
    let tag_names = load_tag_names(pool, exhibit.id).await?;
    let caps = format.capabilities();
    let format_display = format.display_name().to_string();
    let html = EditPage {
        page_title: format!("Edit: {}", exhibit.title),
        csrf_token: csrf::get_or_create(session)?,
        user_id: userid.into(),
        flash: flash::take(session),
        exhibit,
        sections,
        formats,
        tag_names,
        caps,
        format_display,
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

#[post("/exhibits/{id}/edit")]
async fn edit_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    form: web::Form<ExhibitForm>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    let id = path.into_inner();
    if let Err(e) = update_exhibit(pool.get_ref(), id, &form).await {
        return actix_web::ResponseError::error_response(&e);
    }
    // Two-button form: regular Save stays on the exhibits list,
    // "Save & add media" jumps straight into the upload page for the
    // freshly saved exhibit.
    let (msg, dest) = if form.next == "media" {
        (
            "Saved — now add some media.",
            format!("/admin/exhibits/{id}/media"),
        )
    } else {
        ("Exhibit saved", "/admin/exhibits".to_string())
    };
    flash::redirect(&session, msg, dest)
}

#[get("/exhibits/{id}/confirm-delete")]
async fn confirm_delete_get(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
) -> HttpResponse {
    let user = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    match confirm_inner(&session, pool.get_ref(), &user.userid, path.into_inner()).await {
        Ok(r) => r,
        Err(e) => actix_web::ResponseError::error_response(&e),
    }
}

async fn confirm_inner(
    session: &Session,
    pool: &PgPool,
    userid: &str,
    id: i32,
) -> AppResult<HttpResponse> {
    let exhibit = sqlx::query_as::<_, Exhibit>("SELECT * FROM exhibits WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let html = ConfirmDelete {
        page_title: format!("Delete: {}", exhibit.title),
        csrf_token: csrf::get_or_create(session)?,
        user_id: userid.into(),
        flash: flash::take(session),
        exhibit,
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

#[post("/exhibits/{id}/delete")]
async fn delete_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    form: web::Form<CsrfOnly>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    let id = path.into_inner();
    if let Err(e) = sqlx::query("DELETE FROM exhibits WHERE id = $1")
        .bind(id)
        .execute(pool.get_ref())
        .await
    {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    // Best-effort cascade for media rows.
    let _ = sqlx::query("DELETE FROM media WHERE ref_id = $1 AND obj_type = 'exhibits'")
        .bind(id)
        .execute(pool.get_ref())
        .await;
    flash::redirect(&session, "Exhibit deleted", "/admin/exhibits")
}

#[post("/exhibits/reorder")]
async fn reorder_post(
    session: Session,
    pool: web::Data<PgPool>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    let token = form.get("_csrf").cloned().unwrap_or_default();
    if !csrf::verify(&session, &token) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
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
        if let Err(e) = sqlx::query("UPDATE exhibits SET ord = $1 WHERE id = $2")
            .bind(ord)
            .bind(id)
            .execute(&mut *tx)
            .await
        {
            return actix_web::ResponseError::error_response(&AppError::Db(e));
        }
    }
    if let Err(e) = tx.commit().await {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    flash::redirect(&session, "Order saved", "/admin/exhibits")
}

#[post("/exhibits/{id}/move-up")]
async fn move_up_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    form: web::Form<CsrfOnly>,
) -> HttpResponse {
    reorder(session, pool, path, form, true).await
}

#[post("/exhibits/{id}/move-down")]
async fn move_down_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    form: web::Form<CsrfOnly>,
) -> HttpResponse {
    reorder(session, pool, path, form, false).await
}

async fn reorder(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    form: web::Form<CsrfOnly>,
    up: bool,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    let id = path.into_inner();
    if let Err(e) = swap_with_neighbor(pool.get_ref(), id, up).await {
        return actix_web::ResponseError::error_response(&e);
    }
    HttpResponse::Found()
        .append_header(("Location", "/admin/exhibits"))
        .finish()
}

async fn swap_with_neighbor(pool: &PgPool, id: i32, up: bool) -> AppResult<()> {
    let exhibit = sqlx::query_as::<_, Exhibit>("SELECT * FROM exhibits WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let neighbor_sql = if up {
        "SELECT * FROM exhibits WHERE section_id = $1 AND ord < $2 ORDER BY ord DESC LIMIT 1"
    } else {
        "SELECT * FROM exhibits WHERE section_id = $1 AND ord > $2 ORDER BY ord ASC LIMIT 1"
    };
    let neighbor: Option<Exhibit> = sqlx::query_as(neighbor_sql)
        .bind(exhibit.section_id)
        .bind(exhibit.ord)
        .fetch_optional(pool)
        .await?;
    let Some(n) = neighbor else { return Ok(()) };

    // If ords are equal, bump the neighbor to make swap meaningful.
    let (new_a, new_b) = if exhibit.ord == n.ord {
        if up {
            (exhibit.ord - 1, exhibit.ord)
        } else {
            (exhibit.ord + 1, exhibit.ord)
        }
    } else {
        (n.ord, exhibit.ord)
    };

    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE exhibits SET ord = $1 WHERE id = $2")
        .bind(new_a)
        .bind(exhibit.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE exhibits SET ord = $1 WHERE id = $2")
        .bind(new_b)
        .bind(n.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Whitelist the schemes we'll let `<a href="…">` see. Admin-controlled, but
/// `javascript:`, `data:`, etc. are rejected because they turn into stored XSS
/// the moment a viewer clicks the nav entry. Returns the trimmed link or an
/// empty string to mean "no external link"; errors only on a non-empty but
/// disallowed scheme.
fn sanitize_external_link(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("tel:")
        || trimmed.starts_with('/')
    {
        return Ok(trimmed.to_string());
    }
    Err(AppError::BadRequest(
        "External link must start with http://, https://, mailto:, tel:, or /".into(),
    ))
}

async fn load_tag_names(pool: &PgPool, exhibit_id: i32) -> AppResult<String> {
    let names: Vec<(String,)> = sqlx::query_as(
        "SELECT g.name FROM tags g
         JOIN tagged t ON t.tag_id = g.id
         WHERE t.obj_id = $1 AND t.obj_type = 'exh'
         ORDER BY g.name",
    )
    .bind(exhibit_id)
    .fetch_all(pool)
    .await?;
    Ok(names
        .into_iter()
        .map(|(n,)| n)
        .collect::<Vec<_>>()
        .join(", "))
}

async fn sync_tags(pool: &PgPool, exhibit_id: i32, raw: &str) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM tagged WHERE obj_id = $1 AND obj_type = 'exh'")
        .bind(exhibit_id)
        .execute(&mut *tx)
        .await?;
    for name in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let row: (i32,) = sqlx::query_as(
            "INSERT INTO tags (name, created_at) VALUES ($1, now())
             ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name
             RETURNING id",
        )
        .bind(name)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO tagged (tag_id, obj_type, obj_id) VALUES ($1, 'exh', $2)
             ON CONFLICT DO NOTHING",
        )
        .bind(row.0)
        .bind(exhibit_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn update_exhibit(pool: &PgPool, id: i32, form: &ExhibitForm) -> AppResult<()> {
    let normalized_url = normalize_url(&form.url);
    let link = sanitize_external_link(&form.link)?;
    let link_target = form.link_target.is_some() && !link.is_empty();
    // Strip `<` so per-exhibit custom CSS can't break out of the inline
    // <style> block it is rendered into (layout.html emits it with |safe).
    let custom_css = crate::markup::sanitize_custom_css(&form.custom_css);

    // Only update password if the field was set (non-empty).
    if form.password.is_empty() {
        sqlx::query(
            "UPDATE exhibits
             SET title = $1, url = $2, section_id = $3, format = $4, thumbs = $5,
                 status = $6, content = $7, is_home = $8, hidden = $9,
                 link = $10, link_target = $11, custom_css = $12,
                 section_top = $13, is_new = $14,
                 theme_text_color = $15, theme_bg_color = $16,
                 updated_at = now()
             WHERE id = $17",
        )
        .bind(&form.title)
        .bind(&normalized_url)
        .bind(form.section_id)
        .bind(&form.format)
        .bind(form.thumbs)
        .bind(form.status)
        .bind(&form.content)
        .bind(form.is_home.is_some())
        .bind(form.hidden.is_some())
        .bind(&link)
        .bind(link_target)
        .bind(&custom_css)
        .bind(form.section_top.is_some())
        .bind(form.is_new.is_some())
        .bind(sanitize_color(&form.theme_text_color))
        .bind(sanitize_color(&form.theme_bg_color))
        .bind(id)
        .execute(pool)
        .await?;
    } else {
        let password_hash =
            crate::auth::hash_password(&form.password).map_err(AppError::Internal)?;
        sqlx::query(
            "UPDATE exhibits
             SET title = $1, url = $2, section_id = $3, format = $4, thumbs = $5,
                 status = $6, content = $7, is_home = $8, hidden = $9,
                 password = $10, link = $11, link_target = $12, custom_css = $13,
                 section_top = $14, is_new = $15,
                 theme_text_color = $16, theme_bg_color = $17,
                 updated_at = now()
             WHERE id = $18",
        )
        .bind(&form.title)
        .bind(&normalized_url)
        .bind(form.section_id)
        .bind(&form.format)
        .bind(form.thumbs)
        .bind(form.status)
        .bind(&form.content)
        .bind(form.is_home.is_some())
        .bind(form.hidden.is_some())
        .bind(&password_hash)
        .bind(&link)
        .bind(link_target)
        .bind(&custom_css)
        .bind(form.section_top.is_some())
        .bind(form.is_new.is_some())
        .bind(sanitize_color(&form.theme_text_color))
        .bind(sanitize_color(&form.theme_bg_color))
        .bind(id)
        .execute(pool)
        .await?;
    }

    // If is_home is set, clear it on every other exhibit (only one home allowed).
    if form.is_home.is_some() {
        sqlx::query("UPDATE exhibits SET is_home = FALSE WHERE id <> $1")
            .bind(id)
            .execute(pool)
            .await?;
    }
    sync_tags(pool, id, &form.tag_names).await?;
    Ok(())
}

/// Whitelist what we accept from the `<input type="color">` picker
/// before splicing it into the inline `<style>` block. We only emit
/// `#rgb` / `#rrggbb`; anything else (including the empty string) is
/// treated as "no override" so a malicious form post can't inject CSS.
fn sanitize_color(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if !trimmed.starts_with('#') {
        return String::new();
    }
    let hex = &trimmed[1..];
    if (hex.len() == 3 || hex.len() == 6) && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        // Normalize to lower-case so downstream comparisons are stable.
        format!("#{}", hex.to_ascii_lowercase())
    } else {
        String::new()
    }
}

fn normalize_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "/".into();
    }
    let mut s = String::with_capacity(trimmed.len() + 2);
    if !trimmed.starts_with('/') {
        s.push('/');
    }
    s.push_str(trimmed);
    if !s.ends_with('/') {
        s.push('/');
    }
    s
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_get)
        .service(new_get)
        .service(new_post)
        .service(edit_get)
        .service(edit_post)
        .service(confirm_delete_get)
        .service(delete_post)
        .service(reorder_post)
        .service(move_up_post)
        .service(move_down_post);
}

#[cfg(test)]
mod tests {
    use super::{sanitize_color, sanitize_external_link};

    #[test]
    fn color_accepts_valid_hex_only() {
        assert_eq!(sanitize_color("#abcdef"), "#abcdef");
        assert_eq!(sanitize_color("#ABCDEF"), "#abcdef");
        assert_eq!(sanitize_color("#abc"), "#abc");
        assert_eq!(sanitize_color("  #1A2B3C  "), "#1a2b3c");
    }

    #[test]
    fn color_rejects_anything_else() {
        for bad in [
            "",
            "abcdef",
            "#xyz",
            "#1234",
            "red",
            "rgb(0,0,0)",
            "); body{display:none}",
            "#abc; }",
        ] {
            assert_eq!(sanitize_color(bad), "", "should reject: {bad:?}");
        }
    }

    #[test]
    fn empty_link_is_empty() {
        assert_eq!(sanitize_external_link("").unwrap(), "");
        assert_eq!(sanitize_external_link("   ").unwrap(), "");
    }

    #[test]
    fn allowed_schemes() {
        for ok in [
            "https://example.com",
            "http://example.com/path",
            "HTTPS://EXAMPLE.COM",
            "mailto:a@b.com",
            "tel:+15551234567",
            "/relative/path",
        ] {
            assert!(sanitize_external_link(ok).is_ok(), "should allow: {ok}");
        }
    }

    #[test]
    fn rejected_schemes() {
        for bad in [
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "vbscript:msgbox(1)",
            "file:///etc/passwd",
            "example.com", // no scheme, no leading slash
            "../escape",
        ] {
            assert!(sanitize_external_link(bad).is_err(), "should reject: {bad}");
        }
    }

    #[test]
    fn whitespace_trimmed() {
        assert_eq!(
            sanitize_external_link("  https://example.com  ").unwrap(),
            "https://example.com"
        );
    }
}
