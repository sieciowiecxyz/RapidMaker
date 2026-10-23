use super::*;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialInput {
    pub name: String,
    pub density_g_cm3: f64,
    pub cost_net_grosze_per_kg: Option<i64>,
    pub production_enabled: bool,
}
impl MaterialInput {
    pub fn validate(&self) -> Result<(), StorageError> {
        if self.name.trim().is_empty()
            || self.name.len() > 200
            || !self.density_g_cm3.is_finite()
            || !(0.0..10.0).contains(&self.density_g_cm3)
            || self.density_g_cm3 == 0.0
            || self
                .cost_net_grosze_per_kg
                .is_some_and(|v| !(1..=100_000_000).contains(&v))
            || (self.production_enabled && self.cost_net_grosze_per_kg.is_none())
        {
            return Err("invalid material configuration".into());
        }
        Ok(())
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

pub fn validate_color(color: &Color) -> Result<(), StorageError> {
    if !valid_id(&color.code)
        || color.name.trim().is_empty()
        || color.name.len() > 200
        || color.rgba.is_empty()
        || color.rgba.len() > 16
        || color.rgba.iter().any(|v| {
            v.len() != 9 || !v.starts_with('#') || !v[1..].bytes().all(|b| b.is_ascii_hexdigit())
        })
    {
        return Err("invalid color configuration".into());
    }
    Ok(())
}

impl Storage {
    pub async fn save_material(&self, id: &str, input: &MaterialInput) -> Result<(), StorageError> {
        if !valid_id(id) {
            return Err("invalid material identifier".into());
        }
        input.validate()?;
        sqlx::query("INSERT INTO catalog_materials (id,name,density_g_cm3,cost_net_grosze_per_kg,production_enabled,source)
            VALUES ($1,$2,$3,$4,$5,'{\"source\":\"operator\"}'::jsonb)
            ON CONFLICT (id) DO UPDATE SET name=EXCLUDED.name,density_g_cm3=EXCLUDED.density_g_cm3,
            cost_net_grosze_per_kg=EXCLUDED.cost_net_grosze_per_kg,production_enabled=EXCLUDED.production_enabled")
            .bind(id).bind(input.name.trim()).bind(input.density_g_cm3)
            .bind(input.cost_net_grosze_per_kg).bind(input.production_enabled)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn save_color(&self, material_id: &str, color: &Color) -> Result<(), StorageError> {
        if !valid_id(material_id) {
            return Err("invalid material identifier".into());
        }
        validate_color(color)?;
        sqlx::query("INSERT INTO catalog_colors (material_id,code,name,rgba,in_stock) VALUES ($1,$2,$3,$4,$5)
            ON CONFLICT (material_id,code) DO UPDATE SET name=EXCLUDED.name,rgba=EXCLUDED.rgba,in_stock=EXCLUDED.in_stock")
            .bind(material_id).bind(&color.code).bind(color.name.trim()).bind(json!(color.rgba))
            .bind(color.in_stock).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn pricing_config(&self) -> Result<Option<Value>, StorageError> {
        Ok(
            sqlx::query_scalar("SELECT value FROM pricing_config WHERE id=true")
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    /// Require a new version when changing an existing policy; identical retries are idempotent.
    pub async fn save_pricing_config(&self, value: &Value) -> Result<bool, StorageError> {
        if !value.is_object()
            || serde_json::to_vec(value)?.len() > 16_384
            || value["version"]
                .as_str()
                .is_none_or(|v| v.is_empty() || v.len() > 128)
        {
            return Err("invalid pricing configuration".into());
        }
        let result = sqlx::query("INSERT INTO pricing_config (id,value) VALUES (true,$1)
            ON CONFLICT (id) DO UPDATE SET value=EXCLUDED.value
            WHERE pricing_config.value=EXCLUDED.value OR pricing_config.value->>'version' IS DISTINCT FROM EXCLUDED.value->>'version'")
            .bind(value).execute(&self.pool).await?;
        Ok(result.rows_affected() == 1)
    }
}
