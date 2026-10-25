//! Explicit one-time localhost setup. Never run automatically or overwrite an operator's prices.
use rapidmaker_pricing::{Rates, foreign_cost_pln_grosze};
use serde_json::{Value, json};
use sqlx::Row;
use std::time::Duration;

fn fixed(value: &str, places: usize) -> Result<u32, &'static str> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || fraction.len() > places
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return Err("Invalid fixed-point decimal");
    }
    format!("{whole}{fraction:0<places$}")
        .parse()
        .map_err(|_| "Decimal overflow")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if std::env::var("AUTH_MODE").as_deref() != Ok("localhost")
        || !std::env::var("STRIPE_SECRET_KEY")?.starts_with("sk_test_")
    {
        return Err("Only explicit localhost + Stripe test configuration is allowed".into());
    }
    let url = std::env::var("DATABASE_URL")?;
    let parsed = reqwest::Url::parse(&url)?;
    if !matches!(parsed.host_str(), Some("localhost" | "127.0.0.1")) || parsed.port() != Some(18434)
    {
        return Err("Only the local PostgreSQL on 18434 is allowed".into());
    }
    let input: Value = serde_json::from_str(include_str!("../../../data/bambu/pricing-poc.json"))?;
    if input["mode"] != "localhost-poc" || input["currency"] != "EUR" {
        return Err("Explicit EUR POC basis required".into());
    }
    let endpoint = "https://api.nbp.pl/api/exchangerates/rates/a/eur/?format=json";
    let nbp: Value = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?
        .get(endpoint)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let rate = &nbp["rates"][0];
    let date = rate["effectiveDate"].as_str().ok_or("NBP date missing")?;
    if nbp["code"] != "EUR"
        || nbp["table"] != "A"
        || date.len() != 10
        || !date.bytes().all(|b| b.is_ascii_digit() || b == b'-')
    {
        return Err("Invalid NBP response".into());
    }
    let fx = fixed(&rate["mid"].to_string(), 4)?;
    if fx == 0 {
        return Err("Invalid NBP rate".into());
    }
    let mut rates: Rates = serde_json::from_value(input["rates"].clone())?;
    rates.version = format!("poc-bambu-eur-{date}-{fx}");
    rates.validate()?;
    let pool = sqlx::PgPool::connect(&url).await?;
    let mut tx = pool.begin().await?;
    sqlx::query("LOCK TABLE pricing_config, catalog_materials IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let configured: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pricing_config)")
        .fetch_one(&mut *tx)
        .await?;
    if configured {
        return Err("Pricing already exists; use admin settings, nothing overwritten".into());
    }
    let mut results = Vec::new();
    for id in ["GFA00", "GFB00", "GFU01"] {
        if !rapidmaker_slicer::material_supported(id) {
            return Err("Trusted P1P preset missing".into());
        }
        let row = sqlx::query("SELECT cost_net_grosze_per_kg,production_enabled FROM catalog_materials WHERE id=$1 FOR UPDATE")
            .bind(id).fetch_one(&mut *tx).await?;
        if row
            .try_get::<Option<i64>, _>("cost_net_grosze_per_kg")?
            .is_some()
            || row.try_get::<bool, _>("production_enabled")?
        {
            return Err("Operator material configuration exists; nothing overwritten".into());
        }
        let foreign = input["materials"][id]
            .as_str()
            .ok_or("Material price missing")?;
        let cost = foreign_cost_pln_grosze(fixed(foreign, 2)?, fx)?;
        let reference = json!({"mode":"localhost-poc","foreign_net_budget":foreign,"currency":"EUR",
            "currency_basis":input["currency_basis"],"tax_basis":input["tax_basis"],"source":input["source"],
            "nbp_endpoint":endpoint,"nbp_table":rate["no"],"effective_date":date,"rate_x10000":fx,"net_pln_grosze_per_kg":cost});
        sqlx::query("UPDATE catalog_materials SET cost_net_grosze_per_kg=$2,production_enabled=true,source=source||jsonb_build_object('poc_pricing',$3::jsonb) WHERE id=$1")
            .bind(id).bind(i64::from(cost)).bind(reference).execute(&mut *tx).await?;
        results.push(json!({"material_id":id,"net_pln_grosze_per_kg":cost}));
    }
    sqlx::query("INSERT INTO pricing_config(id,value) VALUES(true,$1)")
        .bind(serde_json::to_value(rates)?)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    println!(
        "{}",
        json!({"mode":"localhost-poc","rate_date":date,"rate_x10000":fx,"materials":results})
    );
    Ok(())
}
