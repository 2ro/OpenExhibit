use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use askama::Template;

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)] // Forbidden / BadRequest used by admin routes in phase 2.
pub enum AppError {
    #[error("not found")]
    NotFound,

    #[error("forbidden")]
    Forbidden,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error(transparent)]
    Db(#[from] sqlx::Error),

    #[error(transparent)]
    Template(#[from] askama::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Image(#[from] image::ImageError),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

#[derive(Template)]
#[template(path = "public/error.html")]
struct ErrorPage<'a> {
    code: u16,
    title: &'a str,
    message: &'a str,
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        if matches!(
            self,
            Self::Db(_) | Self::Io(_) | Self::Internal(_) | Self::Template(_) | Self::Image(_)
        ) {
            tracing::error!(error = ?self, "internal error");
        }
        let status = self.status_code();
        let (title, message) = match self {
            Self::NotFound => ("Not found", "The page you're looking for doesn't exist."),
            Self::Forbidden => ("Forbidden", "You don't have permission to view this."),
            Self::BadRequest(_) => ("Bad request", "The request couldn't be processed."),
            _ => (
                "Something went wrong",
                "An internal error occurred. Please try again, or come back later.",
            ),
        };
        let body = ErrorPage {
            code: status.as_u16(),
            title,
            message,
        }
        .render()
        .unwrap_or_else(|_| format!("<h1>{}</h1>", status.canonical_reason().unwrap_or("error")));
        HttpResponse::build(status)
            .content_type("text/html; charset=utf-8")
            .body(body)
    }
}

pub type AppResult<T> = Result<T, AppError>;
