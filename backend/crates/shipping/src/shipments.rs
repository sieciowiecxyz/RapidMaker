//! Paid sandbox shipment operations. Mutations are never retried in this adapter.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MeasuredParcel {
    pub outer_mm: [u32; 3],
    pub weight_g: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedPackage {
    pub reference: String,
    pub body: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShipQuote {
    pub price_grosze: u32,
    pub packages: Vec<PreparedPackage>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShipmentError {
    Invalid(String),
    Unavailable,
    Unauthorized,
    NotFound,
    /// A mutation may have succeeded. Reconcile before any further submission.
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderPackage {
    pub provider_id: String,
    pub reference: String,
    pub tracking_numbers: Vec<String>,
    pub state: String,
    pub body: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandStatus {
    pub state: String,
    pub successful_ids: Vec<String>,
    pub errors: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tracking {
    pub state: String,
    pub numbers: Vec<String>,
    pub parcels: std::collections::BTreeMap<String, String>,
}
fn invalid() -> ShipmentError {
    ShipmentError::Invalid("Sprawdź dane przesyłki i konfigurację konta przewoźnika.".into())
}
fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn reference_valid(value: &str) -> bool {
    safe_id(value) && value.len() <= 22
}
fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
fn map_error(error: Error) -> ShipmentError {
    match error {
        Error::Validation(_) | Error::NoService(_) => invalid(),
        Error::Unavailable(_) => ShipmentError::Unavailable,
    }
}
async fn bounded(
    mut response: Response,
    max: usize,
    mutation: bool,
) -> Result<Vec<u8>, ShipmentError> {
    let failure = || {
        if mutation {
            ShipmentError::Unknown
        } else {
            ShipmentError::Unavailable
        }
    };
    if response.content_length().is_some_and(|n| n > max as u64) {
        return Err(failure());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| failure())? {
        if body.len().saturating_add(chunk.len()) > max {
            return Err(failure());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
impl Sandbox {
    async fn shipment_request(
        &self,
        method: reqwest::Method,
        path: &str,
        version: u8,
        body: Option<&Value>,
        mutation: bool,
    ) -> Result<Value, ShipmentError> {
        let request = self
            .build_request(method, path, version, body)
            .map_err(map_error)?;
        let response = self.client.execute(request).await.map_err(|_| {
            if mutation {
                ShipmentError::Unknown
            } else {
                ShipmentError::Unavailable
            }
        })?;
        let status = response.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(ShipmentError::Unauthorized);
        }
        if status == StatusCode::NOT_FOUND {
            return Err(ShipmentError::NotFound);
        }
        if status == StatusCode::BAD_REQUEST
            || status == StatusCode::UNPROCESSABLE_ENTITY
            || status == StatusCode::CONFLICT
        {
            let value: Value = serde_json::from_slice(&bounded(response, 65536, mutation).await?)
                .map_err(|_| invalid())?;
            if path.starts_with("/order-commands/")
                && value["errors"]
                    .as_array()
                    .is_some_and(|errors| errors.iter().any(|e| e["code"] == "commandNotExists"))
            {
                return Err(ShipmentError::NotFound);
            }
            if value["code"] == "terms_and_conditions_not_valid" {
                return Err(ShipmentError::Invalid(
                    "Zaakceptuj aktualny regulamin przewoźnika w panelu Furgonetki sandbox.".into(),
                ));
            }
            return Err(invalid());
        }
        if !status.is_success() {
            return Err(if mutation {
                ShipmentError::Unknown
            } else {
                ShipmentError::Unavailable
            });
        }
        if status == StatusCode::NO_CONTENT {
            return Ok(Value::Null);
        }
        let bytes = bounded(response, 5 * 1024 * 1024, mutation).await?;
        serde_json::from_slice(&bytes).map_err(|_| {
            if mutation {
                ShipmentError::Unknown
            } else {
                ShipmentError::Unavailable
            }
        })
    }
    /// Validate and price the purchased service, never silently select a cheaper carrier.
    pub async fn quote_shipment(
        &self,
        recipient: &Recipient,
        choice: &DeliveryChoice,
        service_id: &str,
        parcels: &[MeasuredParcel],
        reference: &str,
    ) -> Result<ShipQuote, ShipmentError> {
        if !safe_id(service_id)
            || !reference_valid(reference)
            || parcels.is_empty()
            || parcels.len() > 30
            || choice.method == Method::Pickup
        {
            return Err(invalid());
        }
        let mut recipient = recipient.clone();
        recipient.normalize(choice).map_err(|_| invalid())?;
        let services = self
            .services
            .get(async {
                provider_json(
                    self.request(reqwest::Method::GET, "/account/services", 1, None)
                        .await?,
                )
                .await
            })
            .await
            .map_err(map_error)?;
        let service = services["services"]
            .as_array()
            .ok_or(ShipmentError::Unavailable)?
            .iter()
            .find(|s| id(&s["id"]).as_deref() == Some(service_id))
            .ok_or_else(invalid)?;
        let carrier = service["service"]
            .as_str()
            .ok_or(ShipmentError::Unavailable)?;
        if (choice.method == Method::Locker && carrier != "inpost")
            || (choice.method == Method::Courier
                && !["dpd", "dhl", "ups", "gls", "fedex"].contains(&carrier))
        {
            return Err(invalid());
        }
        let street = if recipient.apartment.is_empty() {
            recipient.street.clone()
        } else {
            format!("{} / {}", recipient.street, recipient.apartment)
        };
        let mut receiver = json!({"name":recipient.name,"email":recipient.email,"phone":recipient.phone,"street":street,"city":recipient.city,"country_code":"PL","postcode":recipient.postal_code});
        if choice.method == Method::Locker {
            let point = self
                .verify_point(choice.point_id.as_deref().ok_or_else(invalid)?)
                .await
                .map_err(map_error)?;
            receiver["point"] = json!(point.code);
            receiver["street"] = json!(point.address.street);
            receiver["city"] = json!(point.address.city);
            receiver["postcode"] = json!(point.address.postcode);
        }
        let group_size = if ["dpd", "dhl", "gls", "fedex"].contains(&carrier) {
            15
        } else {
            1
        };
        let mut result = ShipQuote {
            price_grosze: 0,
            packages: vec![],
        };
        for (index, group) in parcels.chunks(group_size).enumerate() {
            let mut rows = Vec::with_capacity(group.len());
            for parcel in group {
                if parcel.outer_mm.iter().any(|v| *v == 0 || *v > 3000)
                    || parcel.weight_g == 0
                    || parcel.weight_g > 70_000
                {
                    return Err(invalid());
                }
                rows.push(json!({"width":parcel.outer_mm[0].div_ceil(10),"depth":parcel.outer_mm[1].div_ceil(10),"height":parcel.outer_mm[2].div_ceil(10),"weight":f64::from(parcel.weight_g)/1000.,"quantity":1,"type":"package"}));
            }
            let reference = format!("{reference}-{:02}", index + 1);
            let body = json!({"pickup":self.sender,"receiver":receiver,"service_id":provider_id(service_id),"parcels":rows,"user_reference_number":reference});
            self.shipment_request(
                reqwest::Method::POST,
                "/packages/validate",
                2,
                Some(&body),
                false,
            )
            .await?;
            let priced = self.shipment_request(reqwest::Method::POST,"/packages/calculate-price",2,Some(&json!({"package":body,"services":{"service_id":[provider_id(service_id)]}})),false).await?;
            let prices = priced["services_prices"]
                .as_array()
                .ok_or(ShipmentError::Unavailable)?;
            let price = prices
                .iter()
                .find(|row| {
                    id(&row["service_id"]).as_deref() == Some(service_id)
                        && row["available"] == true
                })
                .and_then(|row| money(&row["pricing"]["price_gross"]))
                .ok_or_else(invalid)?;
            result.price_grosze = result.price_grosze.checked_add(price).ok_or_else(invalid)?;
            result.packages.push(PreparedPackage { reference, body });
        }
        Ok(result)
    }
    pub async fn add_shipment(&self, body: &Value) -> Result<ProviderPackage, ShipmentError> {
        if serde_json::to_vec(body).map_or(true, |v| v.len() > 131072) || !body.is_object() {
            return Err(invalid());
        }
        let value = self
            .shipment_request(reqwest::Method::POST, "/packages", 2, Some(body), true)
            .await?;
        let package = parse_package(value).map_err(|_| ShipmentError::Unknown)?;
        if !matches_prepared(&package, body) {
            return Err(ShipmentError::Unknown);
        }
        Ok(package)
    }
    pub async fn order_shipments(
        &self,
        command: &str,
        ids: &[String],
    ) -> Result<(), ShipmentError> {
        if !valid_uuid(command)
            || ids.is_empty()
            || ids.len() > 30
            || ids.iter().any(|id| !safe_id(id))
        {
            return Err(invalid());
        }
        let body = json!({"packages":ids.iter().map(|id|json!({"id":id})).collect::<Vec<_>>(),"request_collection":false,"skip_email_send":true,"label":{"page_format":"a6","file_format":"pdf"}});
        let value = self
            .shipment_request(
                reqwest::Method::PUT,
                &format!("/order-commands/{command}"),
                1,
                Some(&body),
                true,
            )
            .await?;
        if value["uuid"].as_str() != Some(command) {
            return Err(ShipmentError::Unknown);
        }
        Ok(())
    }
    pub async fn shipment_command(&self, command: &str) -> Result<CommandStatus, ShipmentError> {
        if !valid_uuid(command) {
            return Err(invalid());
        }
        let value = self
            .shipment_request(
                reqwest::Method::GET,
                &format!("/order-commands/{command}"),
                1,
                None,
                false,
            )
            .await?;
        parse_command(&value)
    }
    pub async fn get_shipment(&self, provider_id: &str) -> Result<ProviderPackage, ShipmentError> {
        if !safe_id(provider_id) {
            return Err(invalid());
        }
        parse_package(
            self.shipment_request(
                reqwest::Method::GET,
                &format!("/packages/{provider_id}"),
                2,
                None,
                false,
            )
            .await?,
        )
    }
    /// Bounded search; an incomplete or ambiguous search is never proof of absence.
    pub async fn find_shipment_reference(
        &self,
        reference: &str,
    ) -> Result<Option<ProviderPackage>, ShipmentError> {
        if !safe_id(reference) || reference.len() > 25 {
            return Err(invalid());
        }
        let value = self
            .shipment_request(
                reqwest::Method::GET,
                &format!("/packages?limit=100&list_type=all&query={reference}"),
                2,
                None,
                false,
            )
            .await?;
        let rows = value["packages"]
            .as_array()
            .ok_or(ShipmentError::Unavailable)?;
        if rows.len() >= 100 {
            return Err(ShipmentError::Unavailable);
        }
        let matches = rows
            .iter()
            .filter(|p| p["user_reference_number"].as_str() == Some(reference))
            .cloned()
            .collect::<Vec<_>>();
        match matches.len() {
            0 => Ok(None),
            1 => parse_package(
                matches
                    .into_iter()
                    .next()
                    .ok_or(ShipmentError::Unavailable)?,
            )
            .map(Some),
            _ => Err(ShipmentError::Unknown),
        }
    }
    pub async fn shipment_label(
        &self,
        provider_id: &str,
        format: &str,
    ) -> Result<Vec<u8>, ShipmentError> {
        if !safe_id(provider_id) || !["a4", "a6"].contains(&format) {
            return Err(invalid());
        }
        let path = format!("/packages/{provider_id}/label?label%5Bpage_format%5D={format}");
        let response = self
            .request(reqwest::Method::GET, &path, 1, None)
            .await
            .map_err(map_error)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(ShipmentError::Unauthorized);
        }
        if !response.status().is_success() {
            return Err(ShipmentError::Unavailable);
        }
        let pdf = bounded(response, 8 * 1024 * 1024, false).await?;
        if !pdf.starts_with(b"%PDF-") {
            return Err(ShipmentError::Unavailable);
        }
        Ok(pdf)
    }
    pub async fn shipment_tracking(&self, provider_id: &str) -> Result<Tracking, ShipmentError> {
        if !safe_id(provider_id) {
            return Err(invalid());
        }
        let value = self
            .shipment_request(
                reqwest::Method::GET,
                &format!("/packages/{provider_id}/tracking"),
                2,
                None,
                false,
            )
            .await?;
        parse_tracking(&value)
    }
    /// Sandbox-only test diagnostic; this adapter has no production base URL.
    pub async fn simulate_tracking(
        &self,
        provider_id: &str,
        state: &str,
    ) -> Result<(), ShipmentError> {
        if !safe_id(provider_id)
            || ![
                "collected",
                "delivered",
                "returned",
                "transit",
                "delivery-problem",
            ]
            .contains(&state)
        {
            return Err(invalid());
        }
        let package = self.get_shipment(provider_id).await?;
        if package.tracking_numbers.is_empty() {
            return Err(invalid());
        }
        for number in package.tracking_numbers {
            self.shipment_request(reqwest::Method::PUT,"/packages/tracking/simulate",1,Some(&json!({"packages_id":provider_id.parse::<u64>().map_err(|_|invalid())?,"package_no":number,"tracking_state":state})),true).await?;
        }
        Ok(())
    }
}
/// Compare the submitted identity/address and physical parcels, ignoring provider-added metadata.
/// Recovery must never attach a foreign shipment solely because its free-text reference matches.
pub fn matches_prepared(actual: &ProviderPackage, expected: &Value) -> bool {
    fn text(value: &Value) -> Option<String> {
        value
            .as_str()
            .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
    }
    if expected["user_reference_number"].as_str() != Some(actual.reference.as_str())
        || id(&expected["service_id"]) != id(&actual.body["service_id"])
    {
        return false;
    }
    for party in ["pickup", "receiver"] {
        for field in [
            "name",
            "email",
            "street",
            "city",
            "country_code",
            "postcode",
        ] {
            if text(&expected[party][field]) != text(&actual.body[party][field]) {
                return false;
            }
        }
        for field in ["company", "point"] {
            let empty = Value::String(String::new());
            let desired = if expected[party][field].is_null() {
                &empty
            } else {
                &expected[party][field]
            };
            let obtained = if actual.body[party][field].is_null() {
                &empty
            } else {
                &actual.body[party][field]
            };
            if text(desired) != text(obtained) {
                return false;
            }
        }
        let digits = |value: &Value| {
            value
                .as_str()
                .map(|s| s.bytes().filter(u8::is_ascii_digit).collect::<Vec<_>>())
        };
        if digits(&expected[party]["phone"]) != digits(&actual.body[party]["phone"]) {
            return false;
        }
    }
    let Some(want) = expected["parcels"].as_array() else {
        return false;
    };
    let Some(got) = actual.body["parcels"].as_array() else {
        return false;
    };
    if want.is_empty() || want.len() != got.len() || want.len() > 30 {
        return false;
    }
    want.iter().zip(got).all(|(a, b)| {
        ["width", "depth", "height"]
            .iter()
            .all(|key| a[key].as_u64().is_some() && a[key].as_u64() == b[key].as_u64())
            && a["type"] == b["type"]
            && a["weight"]
                .as_f64()
                .zip(b["weight"].as_f64())
                .is_some_and(|(a, b)| a.is_finite() && b.is_finite() && (a - b).abs() < 0.000001)
    })
}
fn parse_package(value: Value) -> Result<ProviderPackage, ShipmentError> {
    let provider_id = id(&value["package_id"])
        .filter(|s| safe_id(s))
        .ok_or(ShipmentError::Unavailable)?;
    let reference = value["user_reference_number"]
        .as_str()
        .filter(|s| s.len() <= 64 && !s.chars().any(char::is_control))
        .ok_or(ShipmentError::Unavailable)?
        .to_owned();
    let state = value["state"]
        .as_str()
        .filter(|s| safe_id(s))
        .ok_or(ShipmentError::Unavailable)?
        .to_owned();
    let mut numbers = vec![];
    for p in value["parcels"].as_array().into_iter().flatten().take(30) {
        if let Some(number) = p["package_no"].as_str().filter(|s| safe_id(s)) {
            numbers.push(number.to_owned());
        }
    }
    if let Some(number) = value["package_no"].as_str().filter(|s| safe_id(s)) {
        numbers.push(number.to_owned());
    }
    numbers.sort();
    numbers.dedup();
    Ok(ProviderPackage {
        provider_id,
        reference,
        state,
        tracking_numbers: numbers,
        body: value,
    })
}
fn parse_command(value: &Value) -> Result<CommandStatus, ShipmentError> {
    let state = match value["status"].as_str() {
        Some("queueing" | "running") => "pending",
        Some("successful") => "succeeded",
        Some("partial_success") => "partial",
        Some("error") => "failed",
        _ => return Err(ShipmentError::Unavailable),
    }
    .into();
    let successful_ids = value["successfully_ordered_packages"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| {
            id(v)
                .filter(|s| safe_id(s))
                .ok_or(ShipmentError::Unavailable)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if successful_ids.len() > 30 {
        return Err(ShipmentError::Unavailable);
    }
    let errors = value["errors"]
        .as_array()
        .into_iter()
        .flatten()
        .take(30)
        .map(|_| "Przewoźnik nie zatwierdził części przesyłek.".into())
        .collect();
    Ok(CommandStatus {
        state,
        successful_ids,
        errors,
    })
}
fn parse_tracking(value: &Value) -> Result<Tracking, ShipmentError> {
    let parcels = value["parcels"]
        .as_object()
        .ok_or(ShipmentError::Unavailable)?;
    if parcels.is_empty() || parcels.len() > 30 {
        return Err(ShipmentError::Unavailable);
    }
    let mut states = vec![];
    let mut numbers = vec![];
    let mut parcel_states = std::collections::BTreeMap::new();
    for (number, parcel) in parcels {
        if !safe_id(number) {
            return Err(ShipmentError::Unavailable);
        }
        numbers.push(number.clone());
        let events = parcel["tracking"]
            .as_array()
            .ok_or(ShipmentError::Unavailable)?;
        if events.len() > 2000 {
            return Err(ShipmentError::Unavailable);
        }
        let mut latest = None;
        for event in events {
            let timestamp = event["datetime"]
                .as_str()
                .and_then(timestamp_seconds)
                .ok_or(ShipmentError::Unavailable)?;
            if latest.is_none_or(|(time, _)| timestamp >= time) {
                latest = Some((timestamp, event));
            }
        }
        let latest = latest.map(|(_, event)| event);
        let state = match latest.and_then(|e| e["state"].as_str()) {
            None | Some("waiting" | "ordered" | "ready-for-transport") => "awaiting_handover",
            Some("delivered") => "delivered",
            Some("returned") => "returned",
            Some("collected" | "transit" | "delivery" | "waiting-for-receiver") => "in_transit",
            _ => "exception",
        };
        states.push(state);
        parcel_states.insert(number.clone(), state.to_owned());
    }
    let state = if states.iter().all(|s| *s == "delivered") {
        "delivered"
    } else if states.contains(&"exception") {
        "exception"
    } else if states.contains(&"returned") {
        "returned"
    } else if states.contains(&"awaiting_handover") {
        "awaiting_handover"
    } else {
        "in_transit"
    };
    Ok(Tracking {
        state: state.into(),
        numbers,
        parcels: parcel_states,
    })
}
// Furgonetka's documented RFC3339 timestamps may straddle the DST offset change.
fn timestamp_seconds(text: &str) -> Option<i64> {
    if !text.is_ascii() || text.len() < 20 {
        return None;
    }
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let year = number(0..4)?;
    let month = number(5..7)?;
    let day = number(8..10)?;
    let hour = number(11..13)?;
    let minute = number(14..16)?;
    let second = number(17..19)?;
    if &text[4..5] != "-"
        || &text[7..8] != "-"
        || &text[10..11] != "T"
        || &text[13..14] != ":"
        || &text[16..17] != ":"
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let offset = if text.ends_with('Z') {
        0
    } else {
        let zone = text.get(text.len().checked_sub(6)?..)?;
        if &zone[3..4] != ":" {
            return None;
        }
        let h = zone[1..3].parse::<i64>().ok()?;
        let m = zone[4..6].parse::<i64>().ok()?;
        if h > 23 || m > 59 {
            return None;
        }
        let sign = match &zone[..1] {
            "+" => 1,
            "-" => -1,
            _ => return None,
        };
        sign * (h * 3600 + m * 60)
    };
    let leap = |y: i64| y / 4 - y / 100 + y / 400;
    let days_before =
        [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334][usize::try_from(month - 1).ok()?];
    let leap_day = i64::from(month > 2 && (year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)));
    let days = (year - 1) * 365 + leap(year - 1) + days_before + leap_day + day - 1;
    Some(days * 86400 + hour * 3600 + minute * 60 + second - offset)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tracking_waits_for_all_parcels() {
        let value = json!({"parcels":{"A":{"tracking":[{"state":"delivered","datetime":"2026-10-24T10:00:00+02:00"}]},"B":{"tracking":[{"state":"ordered","datetime":"2026-10-24T10:00:00+02:00"}]}}});
        assert_eq!(parse_tracking(&value).unwrap().state, "awaiting_handover");
        assert!(parse_tracking(&json!({"parcels":{}})).is_err());
    }
    #[test]
    fn command_and_identifiers_fail_closed() {
        assert_eq!(parse_command(&json!({"status":"partial_success","successfully_ordered_packages":[1],"errors":[{"message":"secret"}]})).unwrap().successful_ids,vec!["1"]);
        assert!(!safe_id("1?redirect=evil"));
        assert!(!valid_uuid("x"));
        assert!(parse_command(&json!({"status":"invented"})).is_err());
    }
    #[test]
    fn latest_tracking_uses_timezone_and_recovery_checks_payload() {
        let value = json!({"parcels":{"A":{"tracking":[
            {"state":"delivered","datetime":"2026-10-25T02:15:00+01:00"},
            {"state":"transit","datetime":"2026-10-25T02:45:00+02:00"}
        ]}}});
        assert_eq!(parse_tracking(&value).unwrap().state, "delivered");
        let expected = json!({"pickup":{"name":"Sender","email":"sender@example.com","phone":"+48123456789","street":"Street 1","city":"Warszawa","postcode":"00-001","country_code":"PL"},"receiver":{"name":"Client","email":"client@example.com","phone":"123456789","street":"Street 2","city":"Warszawa","postcode":"00-001","country_code":"PL"},"service_id":1,"user_reference_number":"RM-123-01","parcels":[{"width":20,"depth":10,"height":5,"weight":0.5,"type":"package","quantity":1}]});
        let mut returned = expected.clone();
        returned["package_id"] = json!(12);
        returned["state"] = json!("waiting");
        returned["parcels"][0]["quantity"] = Value::Null;
        let mut actual = parse_package(returned).unwrap();
        assert!(matches_prepared(&actual, &expected));
        actual.body["parcels"][0]["weight"] = json!(1.);
        assert!(!matches_prepared(&actual, &expected));
        actual.body = expected.clone();
        actual.body["receiver"]["email"] = json!("other@example.com");
        assert!(!matches_prepared(&actual, &expected));
    }
}
