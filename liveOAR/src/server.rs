use std::sync::{Arc, Mutex, RwLock};

use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Local, TimeZone};
use serde::Deserialize;

use crate::api_types::ApiSnapshot;
use crate::oar_fetch::{get_current_jobs_for_period, get_jobs_from_json, get_resources_from_json};

/// Written outside liveOAR/src so trunk's file watcher never sees it.
const DATA_PATH: &str = "/tmp/liveOAR_data.json";

#[derive(Clone)]
struct ServerState {
    snapshot: Arc<RwLock<ApiSnapshot>>,
    /// Current view window; updated by the WASM client on every fetch.
    window: Arc<Mutex<(DateTime<Local>, DateTime<Local>)>>,
    ssh_host: Arc<String>,
}

#[derive(Deserialize)]
struct WindowQuery {
    start: Option<i64>,
    end: Option<i64>,
}

fn apply_window(state: &ServerState, params: &WindowQuery) {
    if let (Some(s), Some(e)) = (params.start, params.end) {
        if let (Some(start), Some(end)) = (
            Local.timestamp_opt(s, 0).single(),
            Local.timestamp_opt(e, 0).single(),
        ) {
            *state.window.lock().unwrap() = (start, end);
        }
    }
}

fn do_ssh_fetch(state: &ServerState) {
    let (start, end) = *state.window.lock().unwrap();
    if get_current_jobs_for_period(start, end, &state.ssh_host, DATA_PATH) {
        let jobs = get_jobs_from_json(DATA_PATH);
        let resources = get_resources_from_json(DATA_PATH);
        if let Ok(mut snap) = state.snapshot.write() {
            snap.jobs = jobs;
            snap.resources = resources;
            println!("[server] snapshot updated ({} jobs)", snap.jobs.len());
        }
    } else {
        eprintln!("[server] SSH fetch failed, keeping previous snapshot");
    }
}

/// GET /api/data?start=TS&end=TS
/// Updates the stored view window then returns the current cached snapshot.
async fn get_data(
    Query(params): Query<WindowQuery>,
    State(state): State<ServerState>,
) -> Json<ApiSnapshot> {
    apply_window(&state, &params);
    Json(state.snapshot.read().unwrap().clone())
}

/// POST /api/refresh?start=TS&end=TS
/// Triggers an immediate SSH fetch (blocks until done) and returns fresh data.
/// Mirrors what the native `instant_update` does.
async fn post_refresh_impl(
    Query(params): Query<WindowQuery>,
    State(state): State<ServerState>,
) -> Json<ApiSnapshot> {
    apply_window(&state, &params);
    let state_clone = state.clone();
    tokio::task::spawn_blocking(move || do_ssh_fetch(&state_clone))
        .await
        .ok();
    Json(state.snapshot.read().unwrap().clone())
}

pub async fn run(ssh_host: String, port: u16) {
    let now = Local::now();
    let state = ServerState {
        snapshot: Arc::new(RwLock::new(ApiSnapshot::default())),
        window: Arc::new(Mutex::new((
            now - chrono::Duration::hours(12),
            now + chrono::Duration::hours(12),
        ))),
        ssh_host: Arc::new(ssh_host),
    };

    // Background auto-refresh every 30 s using whatever window the client last set.
    {
        let state = state.clone();
        std::thread::spawn(move || loop {
            do_ssh_fetch(&state);
            std::thread::sleep(std::time::Duration::from_secs(30));
        });
    }

    let app = Router::new()
        .route("/api/data", get(get_data))
        .route("/api/refresh", post(post_refresh_impl))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind {}: {}", addr, e));
    println!("[server] listening on http://{}", addr);
    axum::serve(listener, app).await.unwrap();
}
