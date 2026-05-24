use actix_session::Session;
use actix_web::{get, post, web, HttpResponse};
use askama::Template;
use serde::Deserialize;
use sqlx::PgPool;

use crate::auth;
use crate::csrf;
use crate::error::{AppError, AppResult};
use crate::flash;
use crate::models::tag::Tag;

#[derive(Template)]
#[template(path = "admin/tags/list.html")]
struct ListPage {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    tags: Vec<Tag>,
}

#[derive(Template)]
#[template(path = "admin/tags/edit.html")]
struct EditPage {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    tag: Tag,
}

#[derive(Template)]
#[template(path = "admin/tags/confirm_delete.html")]
struct ConfirmDelete {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    tag: Tag,
}

#[derive(Deserialize)]
struct TagForm {
    #[serde(rename = "_csrf")]
    csrf: String,
    name: String,
    grp: i16,
}

#[derive(Deserialize)]
struct CsrfOnly {
    #[serde(rename = "_csrf")]
    csrf: String,
}

#[get("/tags")]
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
    let tags = sqlx::query_as::<_, Tag>("SELECT * FROM tags ORDER BY name")
        .fetch_all(pool)
        .await?;
    let html = ListPage {
        page_title: "Tags".into(),
        csrf_token: csrf::get_or_create(session)?,
        user_id: userid.into(),
        flash: flash::take(session),
        tags,
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

#[get("/tags/new")]
async fn new_get(session: Session, pool: web::Data<PgPool>) -> HttpResponse {
    let user = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let token = match csrf::get_or_create(&session) {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let html = (EditPage {
        page_title: "New tag".into(),
        csrf_token: token,
        user_id: user.userid,
        flash: flash::take(&session),
        tag: blank_tag(),
    })
    .render();
    match html {
        Ok(h) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(h),
        Err(e) => actix_web::ResponseError::error_response(&AppError::Template(e)),
    }
}

#[post("/tags/new")]
async fn new_post(
    session: Session,
    pool: web::Data<PgPool>,
    form: web::Form<TagForm>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    if let Err(e) = sqlx::query("INSERT INTO tags (name, grp, created_at) VALUES ($1, $2, now())")
        .bind(&form.name)
        .bind(form.grp)
        .execute(pool.get_ref())
        .await
    {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    flash::redirect(&session, "Tag created", "/admin/tags")
}

#[get("/tags/{id}/edit")]
async fn edit_get(session: Session, pool: web::Data<PgPool>, path: web::Path<i32>) -> HttpResponse {
    let user = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let id = path.into_inner();
    let tag = match sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE id = $1")
        .bind(id)
        .fetch_optional(pool.get_ref())
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return actix_web::ResponseError::error_response(&AppError::NotFound),
        Err(e) => return actix_web::ResponseError::error_response(&AppError::Db(e)),
    };
    let token = match csrf::get_or_create(&session) {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let html = (EditPage {
        page_title: format!("Edit: {}", tag.name),
        csrf_token: token,
        user_id: user.userid,
        flash: flash::take(&session),
        tag,
    })
    .render();
    match html {
        Ok(h) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(h),
        Err(e) => actix_web::ResponseError::error_response(&AppError::Template(e)),
    }
}

#[post("/tags/{id}/edit")]
async fn edit_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    form: web::Form<TagForm>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    if let Err(e) = sqlx::query("UPDATE tags SET name = $1, grp = $2 WHERE id = $3")
        .bind(&form.name)
        .bind(form.grp)
        .bind(path.into_inner())
        .execute(pool.get_ref())
        .await
    {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    flash::redirect(&session, "Tag saved", "/admin/tags")
}

#[get("/tags/{id}/confirm-delete")]
async fn confirm_delete_get(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
) -> HttpResponse {
    let user = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let id = path.into_inner();
    let tag = match sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE id = $1")
        .bind(id)
        .fetch_optional(pool.get_ref())
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return actix_web::ResponseError::error_response(&AppError::NotFound),
        Err(e) => return actix_web::ResponseError::error_response(&AppError::Db(e)),
    };
    let token = match csrf::get_or_create(&session) {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let html = (ConfirmDelete {
        page_title: format!("Delete: {}", tag.name),
        csrf_token: token,
        user_id: user.userid,
        flash: flash::take(&session),
        tag,
    })
    .render();
    match html {
        Ok(h) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(h),
        Err(e) => actix_web::ResponseError::error_response(&AppError::Template(e)),
    }
}

#[post("/tags/{id}/delete")]
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
    if let Err(e) = sqlx::query("DELETE FROM tags WHERE id = $1")
        .bind(path.into_inner())
        .execute(pool.get_ref())
        .await
    {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    flash::redirect(&session, "Tag deleted", "/admin/tags")
}

fn blank_tag() -> Tag {
    Tag {
        id: 0,
        name: String::new(),
        grp: 1,
        created_at: None,
        icon: String::new(),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_get)
        .service(new_get)
        .service(new_post)
        .service(edit_get)
        .service(edit_post)
        .service(confirm_delete_get)
        .service(delete_post);
}
