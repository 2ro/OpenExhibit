use actix_session::Session;
use actix_web::{get, post, web, HttpResponse};
use askama::Template;
use serde::Deserialize;
use sqlx::PgPool;

use crate::auth;
use crate::csrf;
use crate::error::AppError;
use crate::flash;
use crate::models::user::User;

#[derive(Template)]
#[template(path = "admin/users/list.html")]
struct ListPage {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    users: Vec<User>,
    current_user_id: i32,
}

#[derive(Template)]
#[template(path = "admin/users/edit.html")]
struct EditPage {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    user: User,
}

#[derive(Template)]
#[template(path = "admin/users/password.html")]
struct PasswordPage {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    user: User,
}

#[derive(Template)]
#[template(path = "admin/users/confirm_delete.html")]
struct ConfirmDelete {
    page_title: String,
    csrf_token: String,
    user_id: String,
    flash: Option<String>,
    user: User,
}

#[derive(Deserialize)]
struct UserForm {
    #[serde(rename = "_csrf")]
    csrf: String,
    userid: String,
    email: String,
    first_name: String,
    last_name: String,
    password: Option<String>,
    is_admin: Option<String>,
    is_active: Option<String>,
}

#[derive(Deserialize)]
struct PasswordForm {
    #[serde(rename = "_csrf")]
    csrf: String,
    password: String,
    confirm: String,
}

#[derive(Deserialize)]
struct CsrfOnly {
    #[serde(rename = "_csrf")]
    csrf: String,
}

#[get("/users")]
async fn list_get(session: Session, pool: web::Data<PgPool>) -> HttpResponse {
    let me = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let users = match sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY id")
        .fetch_all(pool.get_ref())
        .await
    {
        Ok(u) => u,
        Err(e) => return actix_web::ResponseError::error_response(&AppError::Db(e)),
    };
    let token = match csrf::get_or_create(&session) {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let html = (ListPage {
        page_title: "Users".into(),
        csrf_token: token,
        user_id: me.userid.clone(),
        flash: flash::take(&session),
        users,
        current_user_id: me.id,
    })
    .render();
    match html {
        Ok(h) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(h),
        Err(e) => actix_web::ResponseError::error_response(&AppError::Template(e)),
    }
}

#[get("/users/new")]
async fn new_get(session: Session, pool: web::Data<PgPool>) -> HttpResponse {
    let me = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let token = match csrf::get_or_create(&session) {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let html = (EditPage {
        page_title: "New user".into(),
        csrf_token: token,
        user_id: me.userid,
        flash: flash::take(&session),
        user: blank_user(),
    })
    .render();
    match html {
        Ok(h) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(h),
        Err(e) => actix_web::ResponseError::error_response(&AppError::Template(e)),
    }
}

#[post("/users/new")]
async fn new_post(
    session: Session,
    pool: web::Data<PgPool>,
    form: web::Form<UserForm>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    let password = match &form.password {
        Some(p) if p.len() >= 10 => p.clone(),
        _ => return HttpResponse::BadRequest().body("Password must be at least 10 characters"),
    };
    let hash = match auth::hash_password(&password) {
        Ok(h) => h,
        Err(e) => return actix_web::ResponseError::error_response(&AppError::Internal(e)),
    };
    if let Err(e) = User::insert(
        pool.get_ref(),
        &form.userid,
        &hash,
        &form.email,
        &form.first_name,
        &form.last_name,
        form.is_admin.is_some(),
    )
    .await
    {
        return actix_web::ResponseError::error_response(&e);
    }
    flash::redirect(&session, "User created", "/admin/users")
}

#[get("/users/{id}/edit")]
async fn edit_get(session: Session, pool: web::Data<PgPool>, path: web::Path<i32>) -> HttpResponse {
    let me = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let id = path.into_inner();
    let user = match User::find_by_id(pool.get_ref(), id).await {
        Ok(Some(u)) => u,
        Ok(None) => return actix_web::ResponseError::error_response(&AppError::NotFound),
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let token = match csrf::get_or_create(&session) {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let html = (EditPage {
        page_title: format!("Edit: {}", user.userid),
        csrf_token: token,
        user_id: me.userid,
        flash: flash::take(&session),
        user,
    })
    .render();
    match html {
        Ok(h) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(h),
        Err(e) => actix_web::ResponseError::error_response(&AppError::Template(e)),
    }
}

#[post("/users/{id}/edit")]
async fn edit_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    form: web::Form<UserForm>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    if let Err(e) = sqlx::query(
        "UPDATE users SET userid = $1, email = $2, first_name = $3, last_name = $4,
                          is_admin = $5, is_active = $6 WHERE id = $7",
    )
    .bind(&form.userid)
    .bind(&form.email)
    .bind(&form.first_name)
    .bind(&form.last_name)
    .bind(form.is_admin.is_some())
    .bind(form.is_active.is_some())
    .bind(path.into_inner())
    .execute(pool.get_ref())
    .await
    {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    flash::redirect(&session, "User saved", "/admin/users")
}

#[get("/users/{id}/password")]
async fn password_get(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
) -> HttpResponse {
    let me = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let id = path.into_inner();
    let user = match User::find_by_id(pool.get_ref(), id).await {
        Ok(Some(u)) => u,
        Ok(None) => return actix_web::ResponseError::error_response(&AppError::NotFound),
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let token = match csrf::get_or_create(&session) {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let html = (PasswordPage {
        page_title: format!("Password: {}", user.userid),
        csrf_token: token,
        user_id: me.userid,
        flash: flash::take(&session),
        user,
    })
    .render();
    match html {
        Ok(h) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(h),
        Err(e) => actix_web::ResponseError::error_response(&AppError::Template(e)),
    }
}

#[post("/users/{id}/password")]
async fn password_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    form: web::Form<PasswordForm>,
) -> HttpResponse {
    if let Err(r) = auth::require_admin(&session, pool.get_ref()).await {
        return r;
    }
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    if form.password != form.confirm {
        return HttpResponse::BadRequest().body("Passwords do not match");
    }
    if form.password.len() < 10 {
        return HttpResponse::BadRequest().body("Password must be at least 10 characters");
    }
    let hash = match auth::hash_password(&form.password) {
        Ok(h) => h,
        Err(e) => return actix_web::ResponseError::error_response(&AppError::Internal(e)),
    };
    if let Err(e) = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(&hash)
        .bind(path.into_inner())
        .execute(pool.get_ref())
        .await
    {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    flash::redirect(&session, "Password updated", "/admin/users")
}

#[get("/users/{id}/confirm-delete")]
async fn confirm_delete_get(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
) -> HttpResponse {
    let me = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let id = path.into_inner();
    if id == me.id {
        return HttpResponse::BadRequest().body("Cannot delete the currently logged-in user.");
    }
    let user = match User::find_by_id(pool.get_ref(), id).await {
        Ok(Some(u)) => u,
        Ok(None) => return actix_web::ResponseError::error_response(&AppError::NotFound),
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let token = match csrf::get_or_create(&session) {
        Ok(t) => t,
        Err(e) => return actix_web::ResponseError::error_response(&e),
    };
    let html = (ConfirmDelete {
        page_title: format!("Delete: {}", user.userid),
        csrf_token: token,
        user_id: me.userid,
        flash: flash::take(&session),
        user,
    })
    .render();
    match html {
        Ok(h) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(h),
        Err(e) => actix_web::ResponseError::error_response(&AppError::Template(e)),
    }
}

#[post("/users/{id}/delete")]
async fn delete_post(
    session: Session,
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    form: web::Form<CsrfOnly>,
) -> HttpResponse {
    let me = match auth::require_admin(&session, pool.get_ref()).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    if !csrf::verify(&session, &form.csrf) {
        return HttpResponse::Forbidden().body("Invalid CSRF token");
    }
    let id = path.into_inner();
    if id == me.id {
        return HttpResponse::BadRequest().body("Cannot delete the currently logged-in user.");
    }
    if let Err(e) = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(pool.get_ref())
        .await
    {
        return actix_web::ResponseError::error_response(&AppError::Db(e));
    }
    flash::redirect(&session, "User deleted", "/admin/users")
}

fn blank_user() -> User {
    User {
        id: 0,
        userid: String::new(),
        password_hash: String::new(),
        email: String::new(),
        threads: 10,
        writing: false,
        offset_: 0,
        date_format: "%d %B %Y".into(),
        lang: "en-us".into(),
        user_hash: String::new(),
        help: false,
        mode: 0,
        first_name: String::new(),
        last_name: String::new(),
        is_admin: true,
        is_active: true,
        is_client: false,
        img: String::new(),
        reset_token: None,
        reset_expires: None,
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_get)
        .service(new_get)
        .service(new_post)
        .service(edit_get)
        .service(edit_post)
        .service(password_get)
        .service(password_post)
        .service(confirm_delete_get)
        .service(delete_post);
}
