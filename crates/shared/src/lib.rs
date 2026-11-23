use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Event {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub category: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub venue_name: String,
    pub address: String,
    pub lat: f64,
    pub lng: f64,
    pub price_text: String,
    pub is_free: bool,
    pub organizer: String,
    pub contact: String,
    pub ticket_url: Option<String>,
    pub image_url: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEvent {
    pub title: String,
    pub description: String,
    pub category: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub venue_name: String,
    pub address: String,
    pub lat: f64,
    pub lng: f64,
    pub price_text: String,
    pub is_free: bool,
    pub organizer: String,
    pub contact: String,
    pub ticket_url: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EventQuery {
    pub west: Option<f64>,
    pub south: Option<f64>,
    pub east: Option<f64>,
    pub north: Option<f64>,
    pub category: Option<String>,
    pub free_only: Option<bool>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

/// A GeoJSON FeatureCollection payload consumed by map.js.
#[derive(Debug, Serialize)]
pub struct GeoJsonFeature {
    pub id: Uuid,
    pub category: String,
    pub title: String,
    pub lat: f64,
    pub lng: f64,
}
