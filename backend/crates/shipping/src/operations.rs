//! Courier pickup and cancellation are asynchronous commands, separate from shipment booking.
use super::*;
use crate::shipments::{safe_id, valid_uuid};

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PickupDate {
    pub date: String,
    pub min_time: String,
    pub max_time: String,
}
fn invalid() -> ShipmentError {
    ShipmentError::Invalid("Sprawdź przesyłki i termin podjazdu kuriera.".into())
}
fn packages(ids: &[String]) -> Result<Value, ShipmentError> {
    if ids.is_empty()
        || ids.len() > 30
        || ids
            .iter()
            .enumerate()
            .any(|(i, id)| !safe_id(id) || ids[..i].contains(id))
    {
        return Err(invalid());
    }
    Ok(json!(
        ids.iter().map(|id| json!({"id":id})).collect::<Vec<_>>()
    ))
}
fn valid_date(value: &str) -> bool {
    let b = value.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || b.iter()
            .enumerate()
            .any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
    {
        return false;
    }
    let year = value[..4].parse::<u32>().unwrap_or(0);
    let month = value[5..7].parse::<usize>().unwrap_or(0);
    let day = value[8..].parse::<u32>().unwrap_or(0);
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    year >= 2000 && day > 0 && month > 0 && month <= 12 && day <= days[month - 1]
}
fn valid_time(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && b.iter()
            .enumerate()
            .all(|(i, b)| i == 2 || b.is_ascii_digit())
        && &value[..2] < "24"
        && &value[3..] < "60"
}
fn command_path(kind: &str, uuid: &str) -> Result<String, ShipmentError> {
    if !valid_uuid(uuid) {
        return Err(invalid());
    }
    let prefix = match kind {
        "pickup" => "pickup-commands",
        "cancel" => "cancel-command",
        _ => return Err(invalid()),
    };
    Ok(format!("/{prefix}/{uuid}"))
}
impl Sandbox {
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn pickup_proposals(
        &self,
        ids: &[String],
        ready_date: &str,
        only_repickup: bool,
    ) -> Result<Value, ShipmentError> {
        if !valid_date(ready_date) {
            return Err(invalid());
        }
        let body = json!({"packages":packages(ids)?,"ready_date":ready_date,"only_repickup":only_repickup,"days_ahead":7});
        self.shipment_request(
            reqwest::Method::POST,
            "/packages/pickup-date-proposals",
            1,
            Some(&body),
            false,
        )
        .await
    }
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn request_pickup(
        &self,
        uuid: &str,
        ids: &[String],
        date: &PickupDate,
    ) -> Result<(), ShipmentError> {
        if !valid_date(&date.date)
            || !valid_time(&date.min_time)
            || !valid_time(&date.max_time)
            || date.min_time >= date.max_time
        {
            return Err(invalid());
        }
        let body = json!({"uuid":uuid,"packages":packages(ids)?,"pickup_date":date});
        self.submit_logistics("pickup", uuid, &body).await
    }
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn cancel_shipments(&self, uuid: &str, ids: &[String]) -> Result<(), ShipmentError> {
        let body = json!({"uuid":uuid,"packages":packages(ids)?});
        self.submit_logistics("cancel", uuid, &body).await
    }
    async fn submit_logistics(
        &self,
        kind: &str,
        uuid: &str,
        body: &Value,
    ) -> Result<(), ShipmentError> {
        let path = command_path(kind, uuid)?;
        let value = self
            .shipment_request(reqwest::Method::PUT, &path, 1, Some(body), true)
            .await?;
        if value["uuid"].as_str() != Some(uuid) {
            return Err(ShipmentError::Unknown);
        }
        Ok(())
    }
    /// Return raw per-package details: acceptance alone is not completed pickup/cancellation.
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn logistics_command(&self, kind: &str, uuid: &str) -> Result<Value, ShipmentError> {
        let path = command_path(kind, uuid)?;
        self.shipment_request(reqwest::Method::GET, &path, 1, None, false)
            .await
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_inputs_are_bounded_unique_and_not_paths() {
        assert!(packages(&[]).is_err());
        assert!(packages(&vec!["1".into(); 31]).is_err());
        assert!(packages(&["1".into(), "1".into()]).is_err());
        assert!(packages(&["../1".into()]).is_err());
        assert_eq!(packages(&["123".into()]).unwrap(), json!([{"id":"123"}]));
        let uuid = "a8bc8db0-8d8b-4f16-ac32-f69be9e40855";
        assert_eq!(
            command_path("cancel", uuid).unwrap(),
            format!("/cancel-command/{uuid}")
        );
        assert!(command_path("delete", uuid).is_err());
        assert!(command_path("pickup", "../x").is_err());
    }
    #[test]
    fn provider_dates_and_times_are_strict() {
        for date in [
            "2026-02-29",
            "2026-00-12",
            "2026-04-31",
            "2026-13-01",
            "2026-1-01",
            "😀-01-01",
        ] {
            assert!(!valid_date(date), "{date}");
        }
        assert!(valid_date("2028-02-29"));
        for time in ["24:00", "12:60", "9:00", "00:0x", "😀"] {
            assert!(!valid_time(time));
        }
        assert!(valid_time("09:30"));
    }
}
