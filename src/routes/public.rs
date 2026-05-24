use std::time::Duration;

use actix_session::Session;
use actix_web::{get, post, web, HttpRequest, HttpResponse};
use askama::Template;
use serde::Deserialize;
use sqlx::PgPool;

use crate::auth;
use crate::csrf;
use crate::error::{AppError, AppResult};
use crate::formats::{self, BaseFields, NavExhibit, NavSection, NavSubsection};
use crate::models::{exhibit::Exhibit, media::Media, section::Section, settings::Settings};
use crate::ratelimit::{self, RateLimiter};
use crate::stats;

// Per-IP limit for the public exhibit password gate. Brute force is the
// real concern here, so keep it tight.
const UNLOCK_MAX_PER_WINDOW: u32 = 8;
const UNLOCK_WINDOW: Duration = Duration::from_secs(60);

#[derive(Template)]
#[template(path = "public/password_gate.html")]
struct PasswordGate {
    site_lang: String,
    obj_name: String,
    exhibit_title: String,
    exhibit_url: String,
    csrf_token: String,
    error: bool,
}

#[derive(Deserialize)]
struct PasswordForm {
    #[serde(rename = "_csrf")]
    csrf: String,
    password: String,
}

#[derive(Template)]
#[template(path = "public/tag.html")]
struct TagPage {
    base: BaseFields,
    tag_name: String,
    exhibits: Vec<TaggedExhibit>,
}

pub struct TaggedExhibit {
    #[allow(dead_code)] // referenced by future tag templates; kept for symmetry with rows.
    pub id: i32,
    pub title: String,
    pub url: String,
    pub thumb_url: Option<String>,
}

#[get("/")]
async fn home(
    req: HttpRequest,
    session: Session,
    pool: web::Data<PgPool>,
) -> AppResult<HttpResponse> {
    let settings = Settings::load(pool.get_ref()).await?;
    let exhibit = Exhibit::find_home(pool.get_ref())
        .await?
        .ok_or(AppError::NotFound)?;
    stats::log_hit(&req, pool.get_ref(), "/");
    gated_render(pool.get_ref(), &session, &settings, exhibit, false).await
}

#[get("/{path:.*}")]
async fn catch_all(
    req: HttpRequest,
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<String>,
) -> AppResult<HttpResponse> {
    let raw = path.into_inner();
    let normalized = format!("/{}", raw.trim_end_matches('/'));

    let settings = Settings::load(pool.get_ref()).await?;

    if let Some(exhibit) = Exhibit::find_by_url(pool.get_ref(), &format!("{normalized}/")).await? {
        stats::log_hit(&req, pool.get_ref(), &exhibit.url);
        if let Some(resp) = formats::find(&exhibit.format).intercept(&exhibit) {
            return Ok(resp);
        }
        return gated_render(pool.get_ref(), &session, &settings, exhibit, false).await;
    }
    if let Some(exhibit) = Exhibit::find_by_url(pool.get_ref(), &normalized).await? {
        stats::log_hit(&req, pool.get_ref(), &exhibit.url);
        if let Some(resp) = formats::find(&exhibit.format).intercept(&exhibit) {
            return Ok(resp);
        }
        return gated_render(pool.get_ref(), &session, &settings, exhibit, false).await;
    }

    Err(AppError::NotFound)
}

#[post("/{path:.*}")]
async fn unlock_post(
    req: HttpRequest,
    session: Session,
    pool: web::Data<PgPool>,
    rl: web::Data<RateLimiter>,
    path: web::Path<String>,
    form: web::Form<PasswordForm>,
) -> AppResult<HttpResponse> {
    // CSRF is checked unconditionally; password gates wouldn't even exist on
    // the page if the form wasn't rendered from this session.
    if !csrf::verify(&session, &form.csrf) {
        return Ok(HttpResponse::Forbidden().body("Invalid CSRF token"));
    }
    if let Some(ip) = ratelimit::peer_ip(&req) {
        if !rl.check("unlock", ip, UNLOCK_MAX_PER_WINDOW, UNLOCK_WINDOW) {
            return Ok(HttpResponse::TooManyRequests()
                .append_header(("Retry-After", "60"))
                .content_type("text/plain; charset=utf-8")
                .body("Too many password attempts. Try again in a minute."));
        }
    }

    let raw = path.into_inner();
    let normalized = format!("/{}", raw.trim_end_matches('/'));
    let settings = Settings::load(pool.get_ref()).await?;
    let exhibit = match Exhibit::find_by_url(pool.get_ref(), &format!("{normalized}/")).await? {
        Some(e) => e,
        None => match Exhibit::find_by_url(pool.get_ref(), &normalized).await? {
            Some(e) => e,
            None => return Err(AppError::NotFound),
        },
    };

    if exhibit.password.is_empty() {
        // Nothing to unlock; just render.
        return gated_render(pool.get_ref(), &session, &settings, exhibit, false).await;
    }
    if auth::verify_password(&form.password, &exhibit.password) {
        session
            .insert(format!("unlocked_exhibit_{}", exhibit.id), true)
            .map_err(|e| AppError::Internal(e.into()))?;
        return gated_render(pool.get_ref(), &session, &settings, exhibit, false).await;
    }
    gated_render(pool.get_ref(), &session, &settings, exhibit, true).await
}

async fn gated_render(
    pool: &PgPool,
    session: &Session,
    settings: &Settings,
    exhibit: Exhibit,
    error: bool,
) -> AppResult<HttpResponse> {
    if !exhibit.password.is_empty() {
        let unlocked: bool = session
            .get(&format!("unlocked_exhibit_{}", exhibit.id))
            .ok()
            .flatten()
            .unwrap_or(false);
        if !unlocked {
            let csrf_token = csrf::get_or_create(session)?;
            let html = PasswordGate {
                site_lang: settings.site_lang.clone(),
                obj_name: settings.obj_name.clone(),
                exhibit_title: exhibit.title.clone(),
                exhibit_url: exhibit.url.clone(),
                csrf_token,
                error,
            }
            .render()?;
            return Ok(HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(html));
        }
    }
    render_exhibit(pool, settings, exhibit).await
}

async fn render_exhibit(
    pool: &PgPool,
    settings: &Settings,
    exhibit: Exhibit,
) -> AppResult<HttpResponse> {
    let media = Media::list_for_exhibit(pool, exhibit.id).await?;
    let nav_sections = build_nav(pool, exhibit.section_id).await?;

    let markup_opts = crate::markup::RenderOptions {
        greentext: settings.enable_greentext,
    };
    let base = BaseFields {
        site_lang: settings.site_lang.clone(),
        page_title: exhibit.title.clone(),
        obj_name: settings.obj_name.clone(),
        description: None,
        body_kind: exhibit.kind.clone(),
        section_id: exhibit.section_id,
        exhibit_id: exhibit.id,
        format: exhibit.format.clone(),
        obj_itop: crate::markup::render_with(
            &substitute_placeholders(&settings.obj_itop, settings),
            markup_opts,
        ),
        obj_ibot: crate::markup::render_with(
            &substitute_placeholders(&settings.obj_ibot, settings),
            markup_opts,
        ),
        nav_sections,
        site_custom_css: settings.custom_css.clone(),
        exhibit_custom_css: exhibit.custom_css.clone(),
        theme_text_color: settings.theme_text_color.clone(),
        theme_bg_color: settings.theme_bg_color.clone(),
    };

    let html = formats::render(&exhibit, &media, base, settings.enable_greentext)?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

fn substitute_placeholders(template: &str, settings: &Settings) -> String {
    // Mirrors the PHP `doVariables()` second-pass substitution.
    template
        .replace("{{ obj_name }}", &settings.obj_name)
        .replace("{{obj_name}}", &settings.obj_name)
        .replace("{{ site_name }}", &settings.site_name)
        .replace("{{site_name}}", &settings.site_name)
}

async fn build_nav(pool: &PgPool, active_section: i16) -> AppResult<Vec<NavSection>> {
    let sections = Section::list_visible(pool).await?;
    let mut out = Vec::with_capacity(sections.len());

    for sec in sections {
        let exhibits = Exhibit::list_for_section(pool, sec.id).await?;

        // Load this section's subsections in display order. The exhibits.section_sub
        // column matches subsections.title (string-keyed in the original schema).
        let subs: Vec<(String,)> = sqlx::query_as(
            "SELECT title FROM subsections WHERE section_id = $1 AND hidden = FALSE \
             ORDER BY ord, id",
        )
        .bind(sec.id)
        .fetch_all(pool)
        .await?;

        let mut top_exhibit: Option<NavExhibit> = None;
        let mut children: Vec<NavExhibit> = Vec::new();
        let mut grouped: std::collections::HashMap<String, Vec<NavExhibit>> =
            std::collections::HashMap::new();

        for ex in exhibits {
            // Ask the format where this exhibit's nav anchor should point.
            // Most formats just return ex.url; external_link returns its
            // configured external URL.
            let href = formats::find(&ex.format).nav_href(&ex);
            let nav_ex = NavExhibit {
                id: ex.id,
                title: ex.title.clone(),
                is_new: ex.is_new,
                href: href.href,
                open_in_new_tab: href.open_in_new_tab,
            };
            if ex.section_top && top_exhibit.is_none() {
                top_exhibit = Some(nav_ex);
            } else if !ex.section_sub.is_empty() {
                grouped
                    .entry(ex.section_sub.clone())
                    .or_default()
                    .push(nav_ex);
            } else {
                children.push(nav_ex);
            }
        }

        let subsections: Vec<NavSubsection> = subs
            .into_iter()
            .filter_map(|(title,)| {
                let exhibits = grouped.remove(&title)?;
                if exhibits.is_empty() {
                    None
                } else {
                    Some(NavSubsection { title, exhibits })
                }
            })
            .collect();

        // Any leftover groups (subsection names without a matching subsections row)
        // get appended at the end so we don't silently drop them.
        let mut subsections = subsections;
        for (title, exhibits) in grouped {
            subsections.push(NavSubsection { title, exhibits });
        }

        // When hide_title is set, the section's top exhibit becomes
        // unreachable via the nav (no heading to click). That's intentional:
        // silent grouping means the heading is the link too. Child exhibits
        // in `children` and `subsections` still render normally.
        out.push(NavSection {
            id: sec.id,
            name: if sec.description.is_empty() {
                sec.name.clone()
            } else {
                sec.description.clone()
            },
            hide_title: sec.hide_title,
            top_exhibit,
            children,
            subsections,
        });
        let _ = active_section; // reserved for future highlighting tweaks
    }
    Ok(out)
}

#[get("/tag/{name}")]
async fn tag_get(
    req: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<String>,
) -> AppResult<HttpResponse> {
    let tag_name = path.into_inner();
    let settings = Settings::load(pool.get_ref()).await?;
    stats::log_hit(&req, pool.get_ref(), &format!("/tag/{tag_name}"));

    // Fetch exhibits tagged with this tag.
    let rows: Vec<(i32, String, String)> = sqlx::query_as(
        "SELECT e.id, e.title, e.url
         FROM exhibits e
         JOIN tagged t ON t.obj_id = e.id AND t.obj_type = 'exh'
         JOIN tags g ON g.id = t.tag_id
         WHERE g.name = $1 AND e.status = 1 AND e.hidden = FALSE
         ORDER BY e.section_id, e.ord, e.id",
    )
    .bind(&tag_name)
    .fetch_all(pool.get_ref())
    .await?;

    let mut exhibits: Vec<TaggedExhibit> = Vec::with_capacity(rows.len());
    for (id, title, url) in rows {
        // Look up the first media of each exhibit for a thumbnail.
        let first: Option<(i32, String)> = sqlx::query_as(
            "SELECT ref_id, file FROM media WHERE ref_id = $1 AND obj_type = 'exhibits' \
             AND hidden = FALSE ORDER BY ord ASC, id ASC LIMIT 1",
        )
        .bind(id)
        .fetch_optional(pool.get_ref())
        .await?;
        let thumb_url =
            first.map(|(ref_id, file)| format!("/files/dimgs/{ref_id}/proportional_200_{file}"));
        exhibits.push(TaggedExhibit {
            id,
            title,
            url,
            thumb_url,
        });
    }

    let nav_sections = build_nav(pool.get_ref(), 0).await?;
    let markup_opts = crate::markup::RenderOptions {
        greentext: settings.enable_greentext,
    };
    let base = BaseFields {
        site_lang: settings.site_lang.clone(),
        page_title: format!("Tag: {tag_name}"),
        obj_name: settings.obj_name.clone(),
        description: None,
        body_kind: "tag".into(),
        section_id: 0,
        exhibit_id: 0,
        format: "tag_display".into(),
        obj_itop: crate::markup::render_with(
            &substitute_placeholders(&settings.obj_itop, &settings),
            markup_opts,
        ),
        obj_ibot: crate::markup::render_with(
            &substitute_placeholders(&settings.obj_ibot, &settings),
            markup_opts,
        ),
        nav_sections,
        site_custom_css: settings.custom_css.clone(),
        exhibit_custom_css: String::new(),
        theme_text_color: settings.theme_text_color.clone(),
        theme_bg_color: settings.theme_bg_color.clone(),
    };

    let html = TagPage {
        base,
        tag_name,
        exhibits,
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(home)
        .service(tag_get)
        .service(unlock_post)
        .service(catch_all);
}
