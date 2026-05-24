use actix_session::Session;
use actix_web::{get, web, HttpResponse, Responder};
use askama::Template;
use sqlx::PgPool;

use crate::auth;
use crate::csrf;
use crate::error::AppResult;
use crate::flash;

#[derive(Template)]
#[template(path = "admin/dashboard.html")]
struct Dashboard {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    exhibits_count: i64,
    sections_count: i64,
    media_count: i64,
    tags_count: i64,
}

#[get("")]
async fn dashboard_get_root(session: Session, pool: web::Data<PgPool>) -> impl Responder {
    dashboard_inner(session, pool).await
}

#[get("/")]
async fn dashboard_get(session: Session, pool: web::Data<PgPool>) -> impl Responder {
    dashboard_inner(session, pool).await
}

async fn dashboard_inner(session: Session, pool: web::Data<PgPool>) -> HttpResponse {
    let user = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    match render(session, pool.get_ref(), &user.userid).await {
        Ok(resp) => resp,
        Err(e) => actix_web::ResponseError::error_response(&e),
    }
}

async fn render(session: Session, pool: &PgPool, userid: &str) -> AppResult<HttpResponse> {
    let exhibits_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM exhibits")
        .fetch_one(pool)
        .await?;
    let sections_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sections")
        .fetch_one(pool)
        .await?;
    let media_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM media")
        .fetch_one(pool)
        .await?;
    let tags_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tags")
        .fetch_one(pool)
        .await?;

    let html = Dashboard {
        page_title: "Dashboard".into(),
        csrf_token: csrf::get_or_create(&session)?,
        user_id: userid.into(),
        flash: flash::take(&session),
        exhibits_count: exhibits_count.0,
        sections_count: sections_count.0,
        media_count: media_count.0,
        tags_count: tags_count.0,
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(dashboard_get_root).service(dashboard_get);
}
