#[derive(Clone, Debug)]
pub enum MarkerShape {
    Circle,
    /// Bar from `timestamp_s` to `end_timestamp_s` — lightweight alternative to a full Job.
    Rectangle { end_timestamp_s: i64 },
}

/// A point-in-time annotation drawn on a Gantt resource row.
/// Produced by file types; rendered generically by the view.
#[derive(Clone, Debug)]
pub struct GanttMarker {
    pub timestamp_s: i64,
    /// Must match the leaf-level key (e.g. hostname) used to group Gantt rows.
    pub resource_name: String,
    pub shape: MarkerShape,
    /// RGBA color bytes — no egui dependency in the data layer.
    pub color: [u8; 4],
    pub tooltip: Option<String>,
}
