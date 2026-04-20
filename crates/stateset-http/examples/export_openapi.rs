use stateset_http::openapi::ApiDoc;
use utoipa::OpenApi;

fn main() {
    let spec = ApiDoc::openapi();
    println!(
        "{}",
        serde_json::to_string_pretty(&spec).expect("OpenAPI spec should serialize to JSON")
    );
}
