//! Private operator metadata. Payment and shipping facts remain owned by their existing modules.
use crate::{Storage, StorageError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use std::collections::HashMap;

#[derive(Debug)]
pub enum ProductionError {
    NotFound,
    Conflict,
    Invalid(&'static str),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    pub model_id: String,
    pub done: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Production {
    pub revision: i64,
    pub stage: String,
    pub due_date: Option<String>,
    pub private_notes: String,
    pub progress: Vec<Progress>,
    pub qc_passed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InvoiceLine {
    pub name: String,
    pub qty: u32,
    pub price_grosze: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InvoiceDraft {
    pub revision: i64,
    pub buyer: String,
    pub address: String,
    pub nip: String,
    pub created: String,
    pub lines: Vec<InvoiceLine>,
}
fn text(value: &str, max: usize) -> bool {
    value.len() <= max
        && !value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
}
fn date(value: &str) -> bool {
    if value.len() != 10
        || !value.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
    {
        return false;
    }
    let Some((y, m, d)) = value[..4]
        .parse::<u32>()
        .ok()
        .zip(value[5..7].parse::<u32>().ok())
        .zip(value[8..].parse::<u32>().ok())
        .map(|((y, m), d)| (y, m, d))
    else {
        return false;
    };
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    (1000..=9999).contains(&y) && d > 0 && d <= days
}
fn quantities(lines: &Value) -> Option<HashMap<String, u32>> {
    let mut quantities = HashMap::new();
    for line in lines.as_array()? {
        let id = line["model_id"].as_str()?;
        let qty = u32::try_from(line["quantity"].as_u64()?).ok()?;
        let total = quantities.entry(id.to_owned()).or_insert(0u32);
        *total = total.checked_add(qty)?;
    }
    Some(quantities)
}
fn default_production(status: &str) -> Production {
    Production {
        revision: 0,
        stage: match status {
            "in_production" => "printing",
            _ => "preparation",
        }
        .into(),
        due_date: None,
        private_notes: String::new(),
        progress: Vec::new(),
        qc_passed: false,
    }
}
fn validate_production(value: &Production, lines: &Value) -> Result<bool, ProductionError> {
    let invalid =
        ProductionError::Invalid("Sprawdź etap, termin, notatkę i liczbę wykonanych sztuk.");
    if value.revision < 0
        || !matches!(
            value.stage.as_str(),
            "preparation" | "printing" | "qc" | "packing"
        )
        || !text(&value.private_notes, 4000)
        || value.due_date.as_deref().is_some_and(|d| !date(d))
        || value.progress.len() > 10
    {
        return Err(invalid);
    }
    let Some(mut quantities) = quantities(lines) else {
        return Err(invalid);
    };
    let mut complete = true;
    for progress in &value.progress {
        let Some(max) = quantities.remove(&progress.model_id) else {
            return Err(invalid);
        };
        if progress.done > max {
            return Err(invalid);
        }
        complete &= progress.done == max;
    }
    complete &= quantities.is_empty();
    if (value.qc_passed || matches!(value.stage.as_str(), "qc" | "packing")) && !complete {
        return Err(ProductionError::Invalid(
            "Ukończ wszystkie sztuki przed kontrolą jakości.",
        ));
    }
    if value.stage == "packing" && !value.qc_passed {
        return Err(ProductionError::Invalid(
            "Pakowanie wymaga zatwierdzonej kontroli jakości.",
        ));
    }
    if value.qc_passed && !matches!(value.stage.as_str(), "qc" | "packing") {
        return Err(ProductionError::Invalid(
            "Cofnięcie do druku wymaga ponownej kontroli jakości.",
        ));
    }
    Ok(complete)
}
fn validate_invoice(value: &InvoiceDraft, amount: i64) -> bool {
    if value.revision < 0
        || !text(&value.buyer, 200)
        || value.buyer.trim().is_empty()
        || !text(&value.address, 500)
        || !date(&value.created)
        || (!value.nip.is_empty()
            && (value.nip.len() != 10 || !value.nip.bytes().all(|b| b.is_ascii_digit())))
        || value.lines.is_empty()
        || value.lines.len() > 30
    {
        return false;
    }
    let mut total = 0i64;
    for line in &value.lines {
        if !text(&line.name, 300)
            || line.name.trim().is_empty()
            || !(1..=1000).contains(&line.qty)
            || !(0..=99999999).contains(&line.price_grosze)
        {
            return false;
        }
        let Some(sum) = line
            .price_grosze
            .checked_mul(i64::from(line.qty))
            .and_then(|v| total.checked_add(v))
        else {
            return false;
        };
        total = sum;
    }
    total == amount
}
impl Storage {
    pub async fn production(&self, id: &str) -> Result<Option<Production>, StorageError> {
        let row=sqlx::query("SELECT o.status,p.value FROM orders o LEFT JOIN order_production p ON p.order_id=o.id WHERE o.id=$1").bind(id).fetch_optional(&self.pool).await?;
        row.map(|r| {
            Ok(match r.try_get::<Option<Value>, _>("value")? {
                Some(v) => serde_json::from_value(v)?,
                None => default_production(&r.try_get::<String, _>("status")?),
            })
        })
        .transpose()
    }
    pub async fn save_production(
        &self,
        id: &str,
        actor: &str,
        mut value: Production,
        now: i64,
    ) -> Result<Result<Production, ProductionError>, StorageError> {
        let mut tx = self.queue_transaction().await?;
        let Some(row) = sqlx::query(
            "SELECT status,paid_at,lines,amount_grosze FROM orders WHERE id=$1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Ok(Err(ProductionError::NotFound));
        };
        let status: String = row.try_get("status")?;
        let old =
            sqlx::query_scalar::<_, Value>("SELECT value FROM order_production WHERE order_id=$1")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
                .map(serde_json::from_value)
                .transpose()?
                .unwrap_or_else(|| default_production(&status));
        if old.revision != value.revision {
            return Ok(Err(ProductionError::Conflict));
        }
        if let Err(e) = validate_production(&value, &row.try_get::<Value, _>("lines")?) {
            return Ok(Err(e));
        }
        if old.stage != value.stage
            || old.progress != value.progress
            || old.qc_passed != value.qc_passed
        {
            let blocked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM shipments WHERE order_id=$1 AND state<>'draft') OR EXISTS(SELECT 1 FROM payment_refunds WHERE order_id=$1 AND state NOT IN ('failed','canceled','succeeded')) OR COALESCE((SELECT sum(amount_grosze) FROM payment_refunds WHERE order_id=$1 AND state='succeeded'),0)>=$2").bind(id).bind(row.try_get::<Option<i64>,_>("amount_grosze")?.unwrap_or(0)).fetch_one(&mut *tx).await?;
            if row.try_get::<Option<i64>, _>("paid_at")?.is_none()
                || !matches!(status.as_str(), "paid" | "in_production")
                || blocked
            {
                return Ok(Err(ProductionError::Invalid(
                    "Postęp wymaga opłaconego zamówienia bez zakończonej produkcji, aktywnego nadania lub nierozstrzygniętego zwrotu.",
                )));
            }
        }
        value.revision += 1;
        sqlx::query("INSERT INTO order_production(order_id,revision,value,updated_at,updated_by) VALUES($1,$2,$3,$4,$5) ON CONFLICT(order_id) DO UPDATE SET revision=excluded.revision,value=excluded.value,updated_at=excluded.updated_at,updated_by=excluded.updated_by").bind(id).bind(value.revision).bind(serde_json::to_value(&value)?).bind(now).bind(actor).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Ok(value))
    }
    pub async fn invoice_draft(
        &self,
        id: &str,
    ) -> Result<Option<Option<InvoiceDraft>>, StorageError> {
        let row=sqlx::query("SELECT d.value FROM orders o LEFT JOIN invoice_drafts d ON d.order_id=o.id WHERE o.id=$1").bind(id).fetch_optional(&self.pool).await?;
        row.map(|r| {
            r.try_get::<Option<Value>, _>("value")?
                .map(serde_json::from_value)
                .transpose()
                .map_err(Into::into)
        })
        .transpose()
    }
    pub async fn save_invoice_draft(
        &self,
        id: &str,
        actor: &str,
        mut value: InvoiceDraft,
        now: i64,
    ) -> Result<Result<InvoiceDraft, ProductionError>, StorageError> {
        let mut tx = self.queue_transaction().await?;
        let Some(row) = sqlx::query("SELECT amount_grosze FROM orders WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
        else {
            return Ok(Err(ProductionError::NotFound));
        };
        let current: Option<i64> =
            sqlx::query_scalar("SELECT revision FROM invoice_drafts WHERE order_id=$1")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if current.unwrap_or(0) != value.revision {
            return Ok(Err(ProductionError::Conflict));
        }
        let amount = row.try_get::<Option<i64>, _>("amount_grosze")?;
        if !amount.is_some_and(|a| validate_invoice(&value, a)) {
            return Ok(Err(ProductionError::Invalid(
                "Sprawdź nabywcę, datę i pozycje. Suma szkicu musi odpowiadać pełnej cenie zamówienia brutto.",
            )));
        }
        value.revision += 1;
        sqlx::query("INSERT INTO invoice_drafts(order_id,revision,value,updated_at,updated_by) VALUES($1,$2,$3,$4,$5) ON CONFLICT(order_id) DO UPDATE SET revision=excluded.revision,value=excluded.value,updated_at=excluded.updated_at,updated_by=excluded.updated_by").bind(id).bind(value.revision).bind(serde_json::to_value(&value)?).bind(now).bind(actor).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Ok(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn production_enforces_ordered_quantities_and_quality_gate() {
        let lines = json!([{"model_id":"a","quantity":2}]);
        let mut p = default_production("paid");
        p.stage = "printing".into();
        p.progress = vec![Progress {
            model_id: "a".into(),
            done: 3,
        }];
        assert!(validate_production(&p, &lines).is_err());
        p.progress[0].done = 2;
        p.stage = "packing".into();
        assert!(validate_production(&p, &lines).is_err());
        p.qc_passed = true;
        assert!(validate_production(&p, &lines).is_ok());
        p.progress.push(p.progress[0].clone());
        assert!(validate_production(&p, &lines).is_err());
        p.progress.pop();
        p.stage = "printing".into();
        assert!(validate_production(&p, &lines).is_err());
        p.qc_passed = false;
        assert!(validate_production(&p, &lines).is_ok());
    }
    #[test]
    fn invoice_is_only_a_bounded_draft_with_the_real_order_total() {
        let mut d = InvoiceDraft {
            revision: 0,
            buyer: "Klient".into(),
            address: String::new(),
            nip: String::new(),
            created: "2026-10-26".into(),
            lines: vec![InvoiceLine {
                name: "Druk".into(),
                qty: 2,
                price_grosze: 1250,
            }],
        };
        assert!(validate_invoice(&d, 2500));
        d.lines[0].price_grosze = 1251;
        assert!(!validate_invoice(&d, 2500));
        assert!(!date("2026-02-29"));
        assert!(date("2028-02-29"));
        assert!(!date("2026-13-01"));
    }
}
