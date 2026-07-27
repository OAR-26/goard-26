use axum::{extract::Query, routing::get, Json, Router};
use chrono::{Local, TimeZone};
use serde::Deserialize;

use crate::api_types::ApiSnapshot;
use crate::oar_fetch::{get_current_jobs_for_period, get_dead_intervals_from_json, get_jobs_from_json, get_resources_from_json, make_oar_version};

const DATA_PATH: &str = "/tmp/liveOAR_data.json";

#[derive(Deserialize)]
struct WindowQuery {
    start: Option<i64>,
    end: Option<i64>,
}

/// GET /api/data?start=TS&end=TS
/// Runs an SSH fetch for the requested interval and returns fresh data.
/// Blocks until the fetch completes — the frontend shows a spinner meanwhile.
async fn get_data(Query(params): Query<WindowQuery>) -> Json<ApiSnapshot> {
    let now = Local::now();
    let start = params.start
        .and_then(|s| Local.timestamp_opt(s, 0).single())
        .unwrap_or(now - chrono::Duration::hours(12));
    let end = params.end
        .and_then(|e| Local.timestamp_opt(e, 0).single())
        .unwrap_or(now + chrono::Duration::hours(12));

    let ssh_host = std::env::var("GOARD_SSH_HOST")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "grenoble.g5k".to_string());

    let version = make_oar_version();
    let snap = tokio::task::spawn_blocking(move || {
        if get_current_jobs_for_period(start, end, &ssh_host, DATA_PATH, &*version) {
            ApiSnapshot {
                jobs: get_jobs_from_json(DATA_PATH, &*version),
                resources: get_resources_from_json(DATA_PATH, &*version),
                dead_intervals: get_dead_intervals_from_json(DATA_PATH),
            }
        } else {
            eprintln!("[server] SSH fetch failed");
            ApiSnapshot::default()
        }
    })
    .await
    .unwrap_or_default();

    Json(snap)
}

pub async fn run(ssh_host: String, port: u16) {
    // Store ssh_host in env so the handler can read it without needing State.
    // (It was already set from the env — this just normalises the value after
    // the --serve flag may have defaulted it.)
    std::env::set_var("GOARD_SSH_HOST", &ssh_host);

    let app = Router::new().route("/api/data", get(get_data));

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind {}: {}", addr, e));
    println!("[server] listening on http://{}", addr);
    axum::serve(listener, app).await.unwrap();
}
