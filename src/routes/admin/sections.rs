use actix_session::Session;
use actix_web::{get, post, web, HttpResponse};
use askama::Template;
use serde::Deserialize;
use sqlx::PgPool;

use crate::auth;
use crate::csrf;
use crate::error::{AppError, AppResult};
use crate::flash;
use crate::models::section::Section;

#[derive(Template)]
#[template(path = "admin/sections/list.html")]
struct ListPage {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    sections: Vec<Section>,
}

#[derive(Template)]
#[template(path = "admin/sections/edit.html")]
struct EditPage {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    section: Section,
}

#[derive(Template)]
#[template(path = "admin/sections/confirm_delete.html")]
struct ConfirmDelete {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    section: Section,
}

#[derive(Deserialize)]
struct SectionForm {
    #[serde(rename = "_csrf")]
    csrf: String,
    name: String,
    path: String,
    description: String,
    ord: i16,
    hidden: Option<String>,
    hide_title: Option<String>,
}

#[derive(Deserialize)]
struct CsrfOnly {
    #[serde(rename = "_csrf")]
    csrf: String,
}

#[get("/sections")]
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
    let sections = sqlx::query_as::<_, Section>("SELECT * FROM sections ORDER BY ord, id")
        .fetch_all(pool)
        .await?;
    let html = ListPage {
        page_title: "Sections".into(),
        csrf_token: csrf::get_or_create(session)?,
        user_id: userid.into(),
        flash: flash::take(session),
        sections,
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

#[get("/sections/new")]
async fn new_get(session: Session, pool: web::Data<PgPool>) -> HttpResponse {
    let user = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let token = match csrf::get_or_create(&session) {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    match (EditPage {
        page_title: "New section".into(),
        csrf_token: token,
        user_id: user.userid,
        flash: flash::take(&session),
        section: blank_section(),
    })
    .render()
    {
        Ok(html) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(html),
        Err(e) => actix_web::ResponseError::error_response(&AppError::Template(e)),
    }
}

#[post("/sections/new")]
async fn new_post(
    session: Session,
    pool: web::Data<PgPool>,
    form: web::Form<SectionForm>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    if let Err(e) = sqlx::query(
        "INSERT INTO sections (name, path, description, ord, hidden, hide_title, kind)
         VALUES ($1, $2, $3, $4, $5, $6, 'exhibits')",
    )
    .bind(&form.name)
    .bind(form.path.trim_end_matches('/'))
    .bind(&form.description)
    .bind(form.ord)
    .bind(form.hidden.is_some())
    .bind(form.hide_title.is_some())
    .execute(pool.get_ref())
    .await
    {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    flash::redirect(&session, "Section created", "/admin/sections")
}

#[get("/sections/{id}/edit")]
async fn edit_get(session: Session, pool: web::Data<PgPool>, path: web::Path<i16>) -> HttpResponse {
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
    id: i16,
) -> AppResult<HttpResponse> {
    let section = sqlx::query_as::<_, Section>("SELECT * FROM sections WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let html = EditPage {
        page_title: format!("Edit: {}", section.name),
        csrf_token: csrf::get_or_create(session)?,
        user_id: userid.into(),
        flash: flash::take(session),
        section,
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

#[post("/sections/{id}/edit")]
async fn edit_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i16>,
    form: web::Form<SectionForm>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    let id = path.into_inner();
    if let Err(e) = sqlx::query(
        "UPDATE sections SET name = $1, path = $2, description = $3, ord = $4,
                             hidden = $5, hide_title = $6
         WHERE id = $7",
    )
    .bind(&form.name)
    .bind(form.path.trim_end_matches('/'))
    .bind(&form.description)
    .bind(form.ord)
    .bind(form.hidden.is_some())
    .bind(form.hide_title.is_some())
    .bind(id)
    .execute(pool.get_ref())
    .await
    {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    flash::redirect(&session, "Section saved", "/admin/sections")
}

#[get("/sections/{id}/confirm-delete")]
async fn confirm_delete_get(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i16>,
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
    id: i16,
) -> AppResult<HttpResponse> {
    let section = sqlx::query_as::<_, Section>("SELECT * FROM sections WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let html = ConfirmDelete {
        page_title: format!("Delete: {}", section.name),
        csrf_token: csrf::get_or_create(session)?,
        user_id: userid.into(),
        flash: flash::take(session),
        section,
    }
    .render()?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

#[post("/sections/{id}/delete")]
async fn delete_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i16>,
    form: web::Form<CsrfOnly>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    let id = path.into_inner();
    if let Err(e) = sqlx::query("DELETE FROM sections WHERE id = $1")
        .bind(id)
        .execute(pool.get_ref())
        .await
    {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    flash::redirect(&session, "Section deleted", "/admin/sections")
}

#[post("/sections/{id}/move-up")]
async fn move_up_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i16>,
    form: web::Form<CsrfOnly>,
) -> HttpResponse {
    reorder(session, pool, path, form, true).await
}

#[post("/sections/{id}/move-down")]
async fn move_down_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i16>,
    form: web::Form<CsrfOnly>,
) -> HttpResponse {
    reorder(session, pool, path, form, false).await
}

async fn reorder(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i16>,
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
    let _ = swap_with_neighbor(pool.get_ref(), id, up).await;
    HttpResponse::Found()
        .append_header(("Location", "/admin/sections"))
        .finish()
}

async fn swap_with_neighbor(pool: &PgPool, id: i16, up: bool) -> AppResult<()> {
    let section = sqlx::query_as::<_, Section>("SELECT * FROM sections WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let neighbor_sql = if up {
        "SELECT * FROM sections WHERE ord < $1 ORDER BY ord DESC LIMIT 1"
    } else {
        "SELECT * FROM sections WHERE ord > $1 ORDER BY ord ASC LIMIT 1"
    };
    let neighbor: Option<Section> = sqlx::query_as(neighbor_sql)
        .bind(section.ord)
        .fetch_optional(pool)
        .await?;
    let Some(n) = neighbor else { return Ok(()) };

    let (a, b) = if section.ord == n.ord {
        if up {
            (section.ord - 1, section.ord)
        } else {
            (section.ord + 1, section.ord)
        }
    } else {
        (n.ord, section.ord)
    };

    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE sections SET ord = $1 WHERE id = $2")
        .bind(a)
        .bind(section.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE sections SET ord = $1 WHERE id = $2")
        .bind(b)
        .bind(n.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

fn blank_section() -> Section {
    Section {
        id: 0,
        name: String::new(),
        kind: "exhibits".into(),
        ord: 10,
        display: 1,
        hidden: false,
        password: String::new(),
        created_at: None,
        path: String::new(),
        description: String::new(),
        proj: 0,
        grp: 0,
        report: false,
        hide_title: false,
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_get)
        .service(new_get)
        .service(new_post)
        .service(edit_get)
        .service(edit_post)
        .service(confirm_delete_get)
        .service(delete_post)
        .service(move_up_post)
        .service(move_down_post);
}
