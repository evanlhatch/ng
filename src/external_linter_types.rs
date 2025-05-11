use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StatixDiagnostic {
    pub message: String,
    pub file: String,
    pub severity: String, // Assuming "error" or "warning"
    pub position: StatixPosition,
}

#[derive(Debug, Deserialize)]
pub struct StatixPosition {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}