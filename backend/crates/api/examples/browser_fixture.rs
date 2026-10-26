//! Disposable browser-test accounts. No production route or authentication bypass.
use rapidmaker_auth::{AuthKey, credentials::*};
use rapidmaker_storage::Storage;
use serde_json::json;
use sqlx::Row;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = std::env::var("DATABASE_URL")?;
    let parsed = reqwest::Url::parse(&url)?;
    if !matches!(parsed.host_str(), Some("localhost" | "127.0.0.1"))
        || !parsed.path().starts_with("/rapidmaker_test_browser_")
        || std::env::var("AUTH_MODE")? != "localhost"
        || std::env::var("AUTH_ORIGIN")? != "http://localhost:18437"
    {
        return Err("Only the disposable browser-test environment is allowed".into());
    }
    let _db = Storage::connect(&url).await?;
    let pool = sqlx::PgPool::connect(&url).await?;
    let database: String = sqlx::query("SELECT current_database() AS name")
        .fetch_one(&pool)
        .await?
        .try_get("name")?;
    if !database.starts_with("rapidmaker_test_browser_") {
        return Err("Not a test database".into());
    }
    let key = AuthKey::from_hex(&std::env::var("AUTH_CODE_KEY")?)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    let password = "Rapidmaker-local-test-2026!";
    let hash = hash_password(password)?;
    let mut accounts = Vec::new();
    for (role, email, use_totp) in [
        (
            "client",
            "void@sieciowiec.xyz",
            std::env::var("TEST_CLIENT_TOTP").as_deref() == Ok("1"),
        ),
        ("admin", "browser-admin@example.invalid", true),
        ("client", "browser-other@example.invalid", true),
        ("client", "browser-security@example.invalid", true),
    ] {
        if email == "void@sieciowiec.xyz" && std::env::var("TEST_REGISTER").as_deref() == Ok("1") {
            continue;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let secret = new_totp_secret()?;
        let sealed = use_totp
            .then(|| seal_totp(&key, &id, &secret))
            .transpose()?;
        sqlx::query("INSERT INTO users(id,email,role,password_hash,totp_secret,created_at) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(&id).bind(email).bind(role).bind(&hash).bind(sealed).bind(now).execute(&pool).await?;
        accounts.push(json!({"id":id,"email":email,"role":role,"password":password,"totp_uri":use_totp.then(||totp_uri(&secret,email)).transpose()?}));
    }
    // Caller writes stdout to a private file; never put this fixture in a report or Git.
    println!("{}", json!({"accounts":accounts}));
    Ok(())
}
