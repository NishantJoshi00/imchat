use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct AMResponse {
    pub status: String,
    pub elapsed: f64,
}
