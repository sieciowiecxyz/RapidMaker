//! Read-only reference generated from actual router registrations at build time.
use axum::{
    Json, Router,
    http::{HeaderMap, HeaderValue, header},
    response::Html,
    routing::get,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

include!(concat!(env!("OUT_DIR"), "/api_routes.rs"));

const GROUPS: [(&str, &str, &str); 8] = [
    (
        "/api/auth",
        "Logowanie",
        "Kody email, sesje, passkey i przypisanie plików gościa.",
    ),
    (
        "/api/models",
        "Modele i podgląd",
        "Upload, analiza, prywatne oryginały i podglądy GLB.",
    ),
    (
        "/api/quotes",
        "Wycena",
        "Trwałe zadania slicera oraz wyniki wycen.",
    ),
    ("/api/catalog", "Katalog", "Materiały, kolory i dostępność."),
    (
        "/api/checkout",
        "Checkout i płatności",
        "Wycena koszyka, konfiguracja i płatności Stripe.",
    ),
    (
        "/api/shipping",
        "Dostawa",
        "Szacowanie paczek i wyszukiwanie punktów odbioru.",
    ),
    (
        "/api/account",
        "Konto klienta",
        "Własne zamówienia, pliki, wiadomości, powiadomienia i status płatności.",
    ),
    (
        "/api/admin",
        "Administracja",
        "Katalog, cennik, zamówienia, pliki, rozmowy i powiadomienia.",
    ),
];

pub fn router() -> Router {
    Router::new()
        .route("/api", get(index))
        .route("/api/routes", get(routes))
}
fn access(path: &str) -> &'static str {
    match path {
        "/api/admin/session" => "Status sesji",
        p if p.starts_with("/api/admin/") => "Admin · MFA",
        "/api"
        | "/api/routes"
        | "/api/auth/start"
        | "/api/catalog/materials"
        | "/api/shipping/estimate"
        | "/api/shipping/points" => "Publiczny",
        "/api/account/notifications" => "Sesja konta",
        "/api/auth/verify" => "Kod + cookie próby",
        "/api/auth/claim-guest" => "Konto + cookie gościa",
        p if p.starts_with("/api/auth/") => "Sesja konta",
        p if p.starts_with("/api/account/") => "Właściciel zamówienia",
        "/api/models" => "Gość lub konto",
        "/api/models/analyze" => "Publiczny",
        p if p.starts_with("/api/models/") || p.starts_with("/api/quotes/") => "Właściciel modelu",
        "/api/checkout/status" | "/api/checkout/config" => "Publiczny",
        p if p.starts_with("/api/checkout/") => "Właściciel koszyka",
        _ => "Sprawdź kontrakt",
    }
}
fn in_group(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}
async fn routes() -> Json<Value> {
    Json(json!({"routes":ROUTES.iter().map(|(method,path)| json!({
        "method":method,"path":path,"access":access(path)
    })).collect::<Vec<_>>()}))
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
async fn index() -> Result<(HeaderMap, Html<String>), super::auth::Error> {
    let nonce = rapidmaker_auth::random_token().map_err(|_| super::auth::unavailable())?;
    let mut sections = String::new();
    for (prefix, title, description) in GROUPS.into_iter().chain([(
        "/api",
        "Dokumentacja i pozostałe",
        "Spis tras aktualnego buildu i pomocnicze endpointy.",
    )]) {
        let rows: Vec<_> = ROUTES
            .iter()
            .filter(|(_, path)| {
                if prefix == "/api" {
                    !GROUPS.iter().any(|(prefix, _, _)| in_group(path, prefix))
                } else {
                    in_group(path, prefix)
                }
            })
            .collect();
        if rows.is_empty() {
            continue;
        }
        sections.push_str(&format!("<section data-title=\"{}\"><h2>{title}<span>{}</span></h2><p>{description}</p><div class=\"table-scroll\"><table><thead><tr><th>Metoda</th><th>Endpoint</th><th>Dostęp</th></tr></thead><tbody>",escape(title),rows.len()));
        for (method, path) in rows {
            sections.push_str(&format!("<tr><td><span class=\"method {method}\">{method}</span></td><td><code>{}</code></td><td class=\"access\">{}</td></tr>", escape(path),escape(access(path))));
        }
        sections.push_str("</tbody></table></div></section>");
    }
    let paths = ROUTES.iter().map(|(_, p)| p).collect::<BTreeSet<_>>().len();
    let html = include_str!("../assets/api.html")
        .replace("{{nonce}}", &nonce)
        .replace("{{sections}}", &sections)
        .replace("{{operations}}", &ROUTES.len().to_string())
        .replace("{{paths}}", &paths.to_string());
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_str(&format!(
        "default-src 'none'; style-src 'nonce-{nonce}'; script-src 'nonce-{nonce}'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"
    )).map_err(|_| super::auth::unavailable())?);
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok((headers, Html(html)))
}
