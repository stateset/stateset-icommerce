use axum::http::StatusCode;

pub async fn health_check() -> StatusCode {
    StatusCode::OK
}

pub async fn ready_check() -> StatusCode {
    StatusCode::OK
}

pub async fn live_check() -> StatusCode {
    StatusCode::OK
}
