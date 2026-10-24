//! PostgreSQL catalog and immutable quote snapshots. Import never overwrites operator settings.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use std::{collections::HashSet, error::Error, time::Duration};

mod admin;
mod auth;
mod chat;
mod checkout;
pub use chat::{InboxLease, InboxOutcome, MessagePage, ReplyRoute};
pub use checkout::{AttemptInput, CheckoutDraft, PaymentAttempt, StripeEvent};
mod email;
mod jobs;
mod models;
pub use jobs::SlicerJob;
mod passkeys;
pub use passkeys::{PasskeyInfo, PasskeyStart};
mod order_files;
mod orders;
pub use admin::{MaterialInput, validate_color};
pub use auth::{AuthSession, AuthUser};
pub use email::{EmailOutcome, EmailStats, QueuedEmail};
pub use models::{StoredModel, account_owner};
pub use orders::OrderInput;

pub type StorageError = Box<dyn Error + Send + Sync>;

#[derive(Clone)]
pub struct Storage {
    pool: PgPool,
}

#[derive(Debug, Serialize)]
pub struct Material {
    pub id: String,
    pub name: String,
    pub density_g_cm3: f64,
    pub cost_net_grosze_per_kg: Option<i64>,
    pub production_enabled: bool,
    pub source: Value,
    pub colors: Vec<Color>,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Color {
    pub code: String,
    pub name: String,
    pub rgba: Vec<String>,
    pub in_stock: bool,
}

#[derive(Deserialize)]
struct Catalog {
    source_version: String,
    printer: Value,
    profiles_sha256: String,
    materials: Vec<ImportedMaterial>,
}

#[derive(Deserialize)]
struct ImportedMaterial {
    id: String,
    name: String,
    density_g_cm3: Vec<String>,
    colors: Vec<Color>,
    #[serde(flatten)]
    source: serde_json::Map<String, Value>,
}

impl Storage {
    pub async fn connect(url: &str) -> Result<Self, StorageError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect(url)
            .await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    /// Re-import source metadata while preserving configured costs, availability and stock.
    pub async fn import_bambu(&self) -> Result<(), StorageError> {
        let catalog = catalog()?;
        let mut transaction = self.pool.begin().await?;
        for material in catalog.materials {
            let density: f64 = material.density_g_cm3[0].parse()?;
            let source = json!({
                "source_version": catalog.source_version,
                "printer": catalog.printer,
                "profiles_sha256": catalog.profiles_sha256,
                "material": material.source,
            });
            sqlx::query(
                "INSERT INTO catalog_materials (id,name,density_g_cm3,source) VALUES ($1,$2,$3,$4)
                 ON CONFLICT (id) DO UPDATE SET source=EXCLUDED.source",
            )
            .bind(&material.id)
            .bind(material.name)
            .bind(density)
            .bind(source)
            .execute(&mut *transaction)
            .await?;
            for color in material.colors {
                sqlx::query(
                    "INSERT INTO catalog_colors (material_id,code,name,rgba) VALUES ($1,$2,$3,$4)
                     ON CONFLICT (material_id,code) DO NOTHING",
                )
                .bind(&material.id)
                .bind(color.code)
                .bind(color.name)
                .bind(json!(color.rgba))
                .execute(&mut *transaction)
                .await?;
            }
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn materials(&self) -> Result<Vec<Material>, StorageError> {
        let rows = sqlx::query(
            "SELECT m.id,m.name,m.density_g_cm3,m.cost_net_grosze_per_kg,m.production_enabled,m.source,
             COALESCE((SELECT jsonb_agg(jsonb_build_object('code',c.code,'name',c.name,
             'rgba',c.rgba,'in_stock',c.in_stock) ORDER BY c.code)
             FROM catalog_colors c WHERE c.material_id=m.id),'[]'::jsonb) AS colors
             FROM catalog_materials m ORDER BY m.name,m.id",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(Material {
                    id: row.try_get("id")?,
                    name: row.try_get("name")?,
                    density_g_cm3: row.try_get("density_g_cm3")?,
                    cost_net_grosze_per_kg: row.try_get("cost_net_grosze_per_kg")?,
                    production_enabled: row.try_get("production_enabled")?,
                    source: row.try_get("source")?,
                    colors: serde_json::from_value(row.try_get("colors")?)?,
                })
            })
            .collect()
    }

    /// Duplicate IDs fail instead of replacing a previously issued quote.
    /// Ownership and expiry are part of the immutable server snapshot.
    pub async fn quote(
        &self,
        id: &str,
        owner: &str,
        now: i64,
    ) -> Result<Option<(Value, Value)>, StorageError> {
        let row = sqlx::query("SELECT request,result FROM quotes WHERE id=$1 AND request->>'owner_hash'=$2 AND (request->>'expires_at')::bigint>$3")
            .bind(id).bind(owner).bind(now).fetch_optional(&self.pool).await?;
        row.map(|row| Ok((row.try_get("request")?, row.try_get("result")?)))
            .transpose()
    }

    pub async fn save_quote(
        &self,
        id: &str,
        request: &Value,
        result: &Value,
    ) -> Result<(), StorageError> {
        if id.is_empty()
            || id.len() > 128
            || !request.is_object()
            || !result.is_object()
            || serde_json::to_vec(request)?.len() > 1_048_576
            || serde_json::to_vec(result)?.len() > 1_048_576
        {
            return Err("invalid quote snapshot".into());
        }
        sqlx::query("INSERT INTO quotes (id,request,result,user_id) VALUES ($1,$2,$3,$4)")
            .bind(id)
            .bind(request)
            .bind(result)
            .bind(request.get("user_id").and_then(Value::as_str))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn catalog() -> Result<Catalog, StorageError> {
    let catalog: Catalog = serde_json::from_str(include_str!("../../../data/bambu/catalog.json"))?;
    let mut material_ids = HashSet::new();
    for material in &catalog.materials {
        let density = material
            .density_g_cm3
            .first()
            .and_then(|s| s.parse::<f64>().ok());
        if !material_ids.insert(&material.id)
            || material.density_g_cm3.len() != 1
            || !density.is_some_and(|d| d.is_finite() && d > 0.0 && d < 10.0)
        {
            return Err("invalid Bambu material identifier or density".into());
        }
        let mut color_codes = HashSet::new();
        for color in &material.colors {
            if !color_codes.insert(&color.code)
                || color.rgba.is_empty()
                || color.rgba.len() > 16
                || color.rgba.iter().any(|rgba| {
                    rgba.len() != 9
                        || !rgba.starts_with('#')
                        || !rgba.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
                })
            {
                return Err("invalid Bambu color identifier or RGBA".into());
            }
        }
    }
    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_is_valid_and_does_not_supply_purchase_prices() {
        let catalog = catalog().unwrap();
        assert_eq!(catalog.materials.len(), 49);
        assert_eq!(
            catalog
                .materials
                .iter()
                .map(|m| m.colors.len())
                .sum::<usize>(),
            314
        );
        assert!(
            catalog
                .materials
                .iter()
                .all(|m| m.source["purchase_price_net_pln_per_kg"].is_null())
        );
    }

    #[tokio::test]
    #[ignore = "requires a dedicated PostgreSQL test database in TEST_DATABASE_URL"]
    async fn postgres_import_preserves_operator_settings_and_quote_ids() {
        let storage = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        storage.import_bambu().await.unwrap();
        sqlx::query("UPDATE catalog_materials SET cost_net_grosze_per_kg=12345, production_enabled=true WHERE id='GFB00'")
            .execute(&storage.pool).await.unwrap();
        sqlx::query(
            "UPDATE catalog_colors SET in_stock=true WHERE material_id='GFB00' AND code='40300'",
        )
        .execute(&storage.pool)
        .await
        .unwrap();
        storage.import_bambu().await.unwrap();
        let reconnected = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        let materials = reconnected.materials().await.unwrap();
        let abs = materials.iter().find(|m| m.id == "GFB00").unwrap();
        assert_eq!(materials.len(), 49);
        assert_eq!(abs.cost_net_grosze_per_kg, Some(12345));
        assert!(abs.production_enabled);
        assert!(
            abs.colors
                .iter()
                .find(|c| c.code == "40300")
                .unwrap()
                .in_stock
        );
        let id = format!(
            "storage-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        storage
            .save_quote(&id, &json!({"quantity": 2}), &json!({"total_grosze": 100}))
            .await
            .unwrap();
        assert!(
            storage
                .save_quote(&id, &json!({}), &json!({}))
                .await
                .is_err()
        );
        sqlx::query("DELETE FROM quotes WHERE id=$1")
            .bind(id)
            .execute(&storage.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE catalog_materials SET cost_net_grosze_per_kg=NULL, production_enabled=false WHERE id='GFB00'")
            .execute(&storage.pool).await.unwrap();
        sqlx::query(
            "UPDATE catalog_colors SET in_stock=false WHERE material_id='GFB00' AND code='40300'",
        )
        .execute(&storage.pool)
        .await
        .unwrap();
    }
}
