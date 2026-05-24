use actix_session::Session;
use actix_web::{get, post, web, HttpResponse};
use askama::Template;
use serde::Deserialize;
use sqlx::PgPool;

use crate::auth;
use crate::config::Config;
use crate::crypto;
use crate::csrf;
use crate::error::{AppError, AppResult};
use crate::flash;
use crate::models::settings::Settings;

#[derive(Template)]
#[template(path = "admin/settings/edit.html")]
struct EditPage {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    settings: Settings,
}

#[derive(Deserialize)]
struct SettingsForm {
    #[serde(rename = "_csrf")]
    csrf: String,
    site_name: String,
    obj_name: String,
    site_lang: String,
    obj_theme: String,
    obj_itop: String,
    obj_ibot: String,
    site_format: String,
    caching: Option<String>,
    tagging: Option<String>,
    #[serde(default)]
    smtp_host: String,
    #[serde(default)]
    smtp_port: String,
    #[serde(default)]
    smtp_user: String,
    #[serde(default)]
    smtp_pass: String,
    #[serde(default)]
    smtp_from: String,
    #[serde(default)]
    custom_css: String,
    #[serde(default)]
    theme_text_color: String,
    #[serde(default)]
    theme_bg_color: String,
    enable_greentext: Option<String>,
}

#[get("/settings")]
async fn edit_get(session: Session, pool: web::Data<PgPool>) -> HttpResponse {
    let me = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    match render(&session, pool.get_ref(), &me.userid).await {
        Ok(r) => r,
        Err(e) => actix_web::ResponseError::error_response(&e),
    }
}

async fn render(session: &Session, pool: &PgPool, userid: &str) -> AppResult<HttpResponse> {
    let settings = Settings::load(pool).await?;
    let html = EditPage {
        page_title: "Settings".into(),
        csrf_token: csrf::get_or_create(session)?,
        user_id: userid.into(),
        flash: flash::take(session),
        settings,
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

#[post("/settings")]
async fn edit_post(
    session: Session,
    pool: web::Data<PgPool>,
    cfg: web::Data<Config>,
    form: web::Form<SettingsForm>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    let smtp_port: i32 = form.smtp_port.trim().parse().unwrap_or(587);
    // Empty smtp_pass means "keep the stored password" — common pattern for
    // password fields that round-trip through HTML forms.
    // Color inputs may arrive empty (left blank) or as `#rrggbb`.
    // We accept either form and let the layout decide whether to emit
    // a :root override at render time.
    let text_color = normalize_color(&form.theme_text_color);
    let bg_color = normalize_color(&form.theme_bg_color);

    let pass_clause = if form.smtp_pass.is_empty() {
        ""
    } else {
        ", smtp_pass = $18"
    };
    let sql = format!(
        "UPDATE settings SET
           site_name = $1, obj_name = $2, site_lang = $3, obj_theme = $4,
           obj_itop = $5, obj_ibot = $6, site_format = $7,
           caching = $8, tagging = $9,
           smtp_host = $10, smtp_port = $11, smtp_user = $12, smtp_from = $13,
           custom_css = $14,
           theme_text_color = $15, theme_bg_color = $16,
           enable_greentext = $17
           {pass_clause}
         WHERE id = 1"
    );
    let encrypted_pass = if form.smtp_pass.is_empty() {
        String::new()
    } else {
        match crypto::encrypt(&form.smtp_pass, &cfg.session_key) {
            Ok(s) => s,
            Err(e) => {
                return actix_web::ResponseError::error_response(&AppError::Internal(e));
            }
        }
    };
    let mut q = sqlx::query(&sql)
        .bind(&form.site_name)
        .bind(&form.obj_name)
        .bind(&form.site_lang)
        .bind(&form.obj_theme)
        .bind(&form.obj_itop)
        .bind(&form.obj_ibot)
        .bind(&form.site_format)
        .bind(form.caching.is_some())
        .bind(form.tagging.is_some())
        .bind(form.smtp_host.trim())
        .bind(smtp_port)
        .bind(form.smtp_user.trim())
        .bind(form.smtp_from.trim())
        .bind(&form.custom_css)
        .bind(&text_color)
        .bind(&bg_color)
        .bind(form.enable_greentext.is_some());
    if !form.smtp_pass.is_empty() {
        q = q.bind(&encrypted_pass);
    }
    if let Err(e) = q.execute(pool.get_ref()).await {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    flash::redirect(&session, "Settings saved", "/admin/settings")
}

/// Accept either `""` (no override) or `#rrggbb` (case-insensitive).
/// Anything else gets coerced to `""` — keeps the column safe from
/// CSS injection via the color field.
fn normalize_color(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        return String::new();
    }
    let bytes = s.as_bytes();
    if bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit) {
        return s.to_ascii_lowercase();
    }
    if bytes.len() == 4 && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit) {
        return s.to_ascii_lowercase();
    }
    String::new()
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(edit_get).service(edit_post);
}

#[cfg(test)]
mod tests {
    use super::normalize_color;

    #[test]
    fn accepts_full_hex() {
        assert_eq!(normalize_color("#0004FF"), "#0004ff");
        assert_eq!(normalize_color("#abc123"), "#abc123");
    }

    #[test]
    fn accepts_short_hex() {
        assert_eq!(normalize_color("#fff"), "#fff");
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(normalize_color("javascript:alert(1)"), "");
        assert_eq!(normalize_color("red"), "");
        assert_eq!(normalize_color("#zzzzzz"), "");
        assert_eq!(normalize_color("rgb(0,0,0)"), "");
    }

    #[test]
    fn empty_passes_through() {
        assert_eq!(normalize_color(""), "");
        assert_eq!(normalize_color("   "), "");
    }
}
