use axum::{
    extract::{Path, Query, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use shared::{Event, EventQuery, NewEvent};
use sqlx::PgPool;
use std::sync::Arc;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::cors::CorsLayer;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    admin_token: String,
}

#[derive(Debug)]
enum AppError {
    BadRequest(String),
    Unauthorized,
    NotFound,
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".into()),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".into()),
            AppError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

fn db_err(e: sqlx::Error) -> AppError {
    tracing::error!("database error: {e}");
    AppError::Internal("database error".into())
}

/// Constant-time string comparison so the admin token cannot be guessed by timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = (a.len() ^ b.len()) as u8;
    let n = a.len().max(b.len());
    for i in 0..n {
        diff |= a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0);
    }
    diff == 0
}

const EVENT_COLUMNS: &str = "id,title,description,category,starts_at,ends_at,venue_name,address,\
     lat,lng,price_text,is_free,organizer,contact,ticket_url,image_url,tags";

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info".into()),
    ).init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let admin_token = std::env::var("ADMIN_TOKEN").unwrap_or_else(|_| "change-me".into());
    let pool = PgPool::connect(&database_url).await.expect("connect to database");
    let state = AppState { pool, admin_token };

    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(2)
            .burst_size(5)
            .finish()
            .expect("governor config"),
    );

    let events_get = get(list_events);
    let events_post = post(submit_event)
        .layer(GovernorLayer::new(governor_conf));

    let admin_router = Router::new()
        .route("/api/admin/events", get(admin_list_events))
        .route("/api/admin/events/{id}/approve", post(admin_set_status_approve))
        .route("/api/admin/events/{id}/reject", post(admin_set_status_reject))
        .route("/api/admin/events/{id}/cancel", post(admin_set_status_cancel))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin));

    let app = Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/events", events_get.merge(events_post))
        .route("/api/events/{id}", get(get_event))
        .merge(admin_router)
        .layer(CorsLayer::permissive()) // tighten before production
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("bind port 8080");
    tracing::info!("API listening on http://0.0.0.0:8080");
    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .await
        .expect("serve");
}

async fn require_admin(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let provided = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .unwrap_or("");
    if !constant_time_eq(provided.as_bytes(), state.admin_token.as_bytes()) {
        // Never log the token.
        return Err(AppError::Unauthorized);
    }
    Ok(next.run(req).await)
}

async fn list_events(
    State(state): State<AppState>,
    Query(q): Query<EventQuery>,
) -> Result<Json<Vec<Event>>, AppError> {
    let rows = sqlx::query_as::<_, Event>(&format!(
        r#"SELECT {EVENT_COLUMNS} FROM events
           WHERE status = 'approved'
             AND ($1::float8 IS NULL OR geom && ST_MakeEnvelope($1,$2,$3,$4,4326)::geography)
             AND ($5::text IS NULL OR category = $5)
             AND ($6::bool IS NULL OR $6 = false OR is_free = true)
             AND ($7::timestamptz IS NULL OR starts_at >= $7)
             AND ($8::timestamptz IS NULL OR starts_at <= $8)
           ORDER BY starts_at LIMIT 500"#
    ))
    .bind(q.west)
    .bind(q.south)
    .bind(q.east)
    .bind(q.north)
    .bind(q.category)
    .bind(q.free_only)
    .bind(q.from)
    .bind(q.to)
    .fetch_all(&state.pool)
    .await
    .map_err(db_err)?;
    Ok(Json(rows))
}

async fn get_event(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Event>, AppError> {
    let row = sqlx::query_as::<_, Event>(&format!(
        "SELECT {EVENT_COLUMNS} FROM events WHERE id = $1 AND status = 'approved'"
    ))
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

fn validate_new_event(n: &NewEvent) -> Result<(), AppError> {
    let title = n.title.trim();
    if title.len() < 3 || title.len() > 120 {
        return Err(AppError::BadRequest("title must be 3-120 characters".into()));
    }
    if n.description.chars().count() > 5000 {
        return Err(AppError::BadRequest("description too long (max 5000)".into()));
    }
    if !(-90.0..=90.0).contains(&n.lat) {
        return Err(AppError::BadRequest("latitude out of range".into()));
    }
    if !(-180.0..=180.0).contains(&n.lng) {
        return Err(AppError::BadRequest("longitude out of range".into()));
    }
    if let Some(end) = n.ends_at {
        if end <= n.starts_at {
            return Err(AppError::BadRequest("ends_at must be after starts_at".into()));
        }
    }
    if let Some(url) = &n.ticket_url {
        if !url.starts_with("https://") {
            return Err(AppError::BadRequest("ticket_url must start with https://".into()));
        }
    }
    if n.tags.len() > 10 {
        return Err(AppError::BadRequest("too many tags (max 10)".into()));
    }
    Ok(())
}

async fn submit_event(
    State(state): State<AppState>,
    Json(payload): Json<NewEvent>,
) -> Result<Json<Event>, AppError> {
    validate_new_event(&payload)?;
    let row = sqlx::query_as::<_, Event>(&format!(
        r#"INSERT INTO events
             (title,description,category,starts_at,ends_at,venue_name,address,lat,lng,
              price_text,is_free,organizer,contact,ticket_url,tags,status)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,'pending')
           RETURNING {EVENT_COLUMNS}"#
    ))
    .bind(payload.title.trim())
    .bind(&payload.description)
    .bind(&payload.category)
    .bind(payload.starts_at)
    .bind(payload.ends_at)
    .bind(&payload.venue_name)
    .bind(&payload.address)
    .bind(payload.lat)
    .bind(payload.lng)
    .bind(&payload.price_text)
    .bind(payload.is_free)
    .bind(&payload.organizer)
    .bind(&payload.contact)
    .bind(&payload.ticket_url)
    .bind(&payload.tags)
    .fetch_one(&state.pool)
    .await
    .map_err(db_err)?;
    Ok(Json(row))
}

#[derive(Debug, Deserialize)]
struct AdminQuery {
    #[serde(default = "default_status")]
    status: String,
}

fn default_status() -> String {
    "pending".into()
}

async fn admin_list_events(
    State(state): State<AppState>,
    Query(q): Query<AdminQuery>,
) -> Result<Json<Vec<Event>>, AppError> {
    const ALLOWED: [&str; 4] = ["pending", "approved", "rejected", "cancelled"];
    if !ALLOWED.contains(&q.status.as_str()) {
        return Err(AppError::BadRequest("invalid status".into()));
    }
    let rows = sqlx::query_as::<_, Event>(&format!(
        "SELECT {EVENT_COLUMNS} FROM events WHERE status::text = $1 ORDER BY created_at DESC LIMIT 500"
    ))
    .bind(&q.status)
    .fetch_all(&state.pool)
    .await
    .map_err(db_err)?;
    Ok(Json(rows))
}

async fn set_status(
    state: &AppState,
    id: Uuid,
    status: &str,
) -> Result<Json<Event>, AppError> {
    let row = sqlx::query_as::<_, Event>(&format!(
        "UPDATE events SET status = $2::event_status WHERE id = $1 RETURNING {EVENT_COLUMNS}"
    ))
    .bind(id)
    .bind(status)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

async fn admin_set_status_approve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Event>, AppError> {
    set_status(&state, id, "approved").await
}

async fn admin_set_status_reject(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Event>, AppError> {
    set_status(&state, id, "rejected").await
}

async fn admin_set_status_cancel(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Event>, AppError> {
    set_status(&state, id, "cancelled").await
}
