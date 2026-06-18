use serde::{Deserialize, Serialize};

const HASH_BYTES: usize = 8192;
const MAX_ENTRIES: usize = 200;
const CACHE_PATH: &str = "simulationGOARD/tab_states.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabViewState {
    pub canvas_width_s: f32,
    pub sideways_pan: f32,
    pub row_height: f32,
    pub view_index: usize,
    pub energy_y_min: Option<f64>,
    pub energy_y_max: Option<f64>,
    pub energy_fit: bool,
    pub energy_panel_height: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    path: Option<String>,
    hash: String,
    state: TabViewState,
}

pub struct TabStateCache {
    entries: Vec<Entry>,
}

impl TabStateCache {
    pub fn load() -> Self {
        let entries = std::fs::read_to_string(CACHE_PATH)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { entries }
    }

    pub fn save_to_disk(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.entries) {
            let _ = std::fs::write(CACHE_PATH, json);
        }
    }

    /// Look up by path first (fast path), then by hash (moved/renamed file fallback).
    pub fn lookup(&mut self, path: Option<&str>, hash: &str) -> Option<TabViewState> {
        if let Some(p) = path {
            if let Some(e) = self.entries.iter_mut().find(|e| e.path.as_deref() == Some(p)) {
                // Update hash in case file content changed but we still match by path
                // (this is safe — the user opened it explicitly).
                // Return the stored state regardless.
                return Some(e.state.clone());
            }
        }
        // Hash fallback: file was moved or renamed.
        if let Some(e) = self.entries.iter_mut().find(|e| e.hash == hash) {
            // Update stored path to new location.
            e.path = path.map(str::to_string);
            return Some(e.state.clone());
        }
        None
    }

    /// Upsert by hash (most stable key).
    pub fn store(&mut self, path: Option<&str>, hash: &str, state: TabViewState) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.hash == hash) {
            e.path = path.map(str::to_string);
            e.state = state;
            return;
        }
        self.entries.push(Entry {
            path: path.map(str::to_string),
            hash: hash.to_string(),
            state,
        });
        // Cap entries — evict oldest (FIFO).
        if self.entries.len() > MAX_ENTRIES {
            self.entries.remove(0);
        }
    }
}

/// FNV-1a 64-bit hash of first 8 KB of content + total length mixed in.
/// No external dependencies; deterministic across platforms.
pub fn compute_file_hash(content: &[u8]) -> String {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;
    let limit = content.len().min(HASH_BYTES);
    let mut hash = FNV_OFFSET;
    for &b in &content[..limit] {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    // Mix total length to distinguish files that share a common prefix.
    for &b in &(content.len() as u64).to_le_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{:016x}", hash)
}
