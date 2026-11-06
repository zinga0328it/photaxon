use serde::Serialize;

#[derive(Serialize)]
pub struct Cabinet {
    pub cabinet_id: String,
    pub status: String,
}
