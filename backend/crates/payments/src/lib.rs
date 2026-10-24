//! Stripe test-mode transport. The caller persists forms/idempotency keys before network I/O.
//! A verified webhook is a notification; reconcile the object before recording payment.
use hmac::{Hmac, Mac};
use reqwest::{Client, Method, Url, redirect::Policy};
use serde::Deserialize;
use serde_json::Value;
use sha2::Sha256;
use std::{fmt, time::Duration};

mod refunds;
pub use refunds::{ExpectedRefund, Refund};

pub const API_VERSION: &str = "2026-09-30.endive";
const API: &str = "https://api.stripe.com/v1";
const MAX_RESPONSE: usize = 512 * 1024;
const MAX_WEBHOOK: usize = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    InvalidInput,
    /// A definite HTTP rejection; response text is deliberately excluded.
    Rejected {
        status: u16,
        code: Option<String>,
    },
    /// A side effect may have happened. Retry only with the persisted idempotency key.
    Unknown,
    /// The response is not the expected payment. Never interpret it as paid or safely absent.
    Mismatch,
    InvalidSignature,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid Stripe request configuration",
            Self::Rejected { .. } => "Stripe rejected request",
            Self::Unknown => "Stripe result unknown",
            Self::Mismatch => "Stripe payment correlation mismatch",
            Self::InvalidSignature => "invalid Stripe webhook signature",
        })
    }
}
impl std::error::Error for Error {}

#[derive(Clone)]
pub struct Stripe {
    client: Client,
    secret: String,
}
#[derive(Clone, Debug)]
pub struct ExpectedPayment {
    pub correlation_id: String,
    pub amount_grosze: i64,
}
pub struct Checkout {
    /// Exact validated provider evidence; server-only and potentially sensitive.
    pub object: Value,
    pub id: String,
    pub url: Option<String>,
    pub status: String,
    pub payment_status: String,
    pub payment_intent: Option<String>,
}
impl Checkout {
    pub fn paid(&self) -> bool {
        self.status == "complete" && self.payment_status == "paid"
    }
}
// Intentionally no Debug: a PaymentIntent can carry a client secret.
pub struct Intent {
    /// Exact validated provider evidence, including any client secret; server-only.
    pub object: Value,
    pub id: String,
    pub status: String,
    pub client_secret: Option<String>,
}
impl Intent {
    pub fn paid(&self) -> bool {
        self.status == "succeeded"
    }
}
#[derive(Debug, Deserialize)]
pub struct WebhookEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub object_id: String,
    pub livemode: bool,
}
impl Stripe {
    pub fn new(secret: String) -> Result<Self, Error> {
        if !(secret.starts_with("sk_test_") || secret.starts_with("rk_test_"))
            || !(16..=256).contains(&secret.len())
            || !secret
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(Error::InvalidInput);
        }
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|_| Error::InvalidInput)?;
        Ok(Self { client, secret })
    }
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn create_checkout(
        &self,
        form: &[(String, String)],
        idempotency_key: &str,
        expected: &ExpectedPayment,
    ) -> Result<Checkout, Error> {
        validate_form(form, expected, true)?;
        let value = self
            .request(
                Method::POST,
                "checkout/sessions",
                Some((form, idempotency_key)),
            )
            .await?;
        checkout(value, expected, None)
    }
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn retrieve_checkout(
        &self,
        id: &str,
        expected: &ExpectedPayment,
    ) -> Result<Checkout, Error> {
        validate_id(id, "cs_test_")?;
        let value = self
            .request(Method::GET, &format!("checkout/sessions/{id}"), None)
            .await?;
        checkout(value, expected, Some(id))
    }
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn expire_checkout(
        &self,
        id: &str,
        idempotency_key: &str,
        expected: &ExpectedPayment,
    ) -> Result<Checkout, Error> {
        validate_id(id, "cs_test_")?;
        let value = self
            .request(
                Method::POST,
                &format!("checkout/sessions/{id}/expire"),
                Some((&[], idempotency_key)),
            )
            .await?;
        let session = checkout(value, expected, Some(id))?;
        if session.status != "expired" {
            return Err(Error::Mismatch);
        }
        Ok(session)
    }
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn create_intent(
        &self,
        form: &[(String, String)],
        idempotency_key: &str,
        expected: &ExpectedPayment,
    ) -> Result<Intent, Error> {
        validate_form(form, expected, false)?;
        let value = self
            .request(
                Method::POST,
                "payment_intents",
                Some((form, idempotency_key)),
            )
            .await?;
        intent(value, expected, None)
    }
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn retrieve_intent(
        &self,
        id: &str,
        expected: &ExpectedPayment,
    ) -> Result<Intent, Error> {
        validate_id(id, "pi_")?;
        let value = self
            .request(Method::GET, &format!("payment_intents/{id}"), None)
            .await?;
        intent(value, expected, Some(id))
    }
    #[tracing::instrument(skip_all, level = "debug", fields(provider = "stripe", operation = tracing::field::Empty))]
    async fn request(
        &self,
        method: Method,
        path: &str,
        form: Option<(&[(String, String)], &str)>,
    ) -> Result<Value, Error> {
        let operation = match path.split('/').next() {
            Some("checkout") => "checkout_session",
            Some("payment_intents") => "payment_intent",
            Some("refunds") => "refund",
            _ => "stripe_request",
        };
        tracing::Span::current().record("operation", operation);
        let started = std::time::Instant::now();
        let result = async {
            let mut request = self
                .client
                .request(method, format!("{API}/{path}"))
                .bearer_auth(&self.secret)
                .header("Stripe-Version", API_VERSION);
            if let Some((form, key)) = form {
                if key.is_empty() || key.len() > 255 || !key.bytes().all(|b| b.is_ascii_graphic()) {
                    return Err(Error::InvalidInput);
                }
                request = request.header("Idempotency-Key", key).form(form);
            }
            let mut response = request.send().await.map_err(|_| Error::Unknown)?;
            let status = response.status();
            tracing::debug!(
                event = "provider_response",
                provider = "stripe",
                operation = operation,
                status = status.as_u16()
            );
            if response
                .content_length()
                .is_some_and(|n| n > MAX_RESPONSE as u64)
            {
                return Err(Error::Unknown);
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| Error::Unknown)? {
                if bytes.len() + chunk.len() > MAX_RESPONSE {
                    return Err(Error::Unknown);
                }
                bytes.extend_from_slice(&chunk);
            }
            let value: Value = serde_json::from_slice(&bytes).map_err(|_| Error::Unknown)?;
            if !status.is_success() {
                return Err(
                    if status.is_client_error() && !matches!(status.as_u16(), 408 | 409 | 429) {
                        Error::Rejected {
                            status: status.as_u16(),
                            code: value["error"]["code"]
                                .as_str()
                                .filter(|s| {
                                    s.len() <= 80
                                        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                                })
                                .map(str::to_owned),
                        }
                    } else {
                        Error::Unknown
                    },
                );
            }
            Ok(value)
        }
        .await;
        if let Err(error) = &result {
            let reason = match error {
                Error::InvalidInput => "invalid_input",
                Error::Rejected { .. } => "provider_rejected",
                Error::Unknown => "unknown_outcome",
                Error::Mismatch => "payment_mismatch",
                Error::InvalidSignature => "invalid_signature",
            };
            tracing::warn!(
                event = "operation_failed",
                provider = "stripe",
                operation = operation,
                elapsed_ms = started.elapsed().as_millis() as u64,
                reason
            );
        } else {
            tracing::debug!(
                event = "operation_completed",
                provider = "stripe",
                operation = operation,
                elapsed_ms = started.elapsed().as_millis() as u64
            );
        }
        result
    }
}
fn validate_id(id: &str, prefix: &str) -> Result<(), Error> {
    if id.len() > prefix.len()
        && id.len() <= 255
        && id.starts_with(prefix)
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        Ok(())
    } else {
        Err(Error::InvalidInput)
    }
}
fn expected_valid(expected: &ExpectedPayment) -> bool {
    expected.amount_grosze > 0
        && expected.amount_grosze <= 99_999_999
        && !expected.correlation_id.is_empty()
        && expected.correlation_id.len() <= 128
        && expected
            .correlation_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn validate_form(
    form: &[(String, String)],
    expected: &ExpectedPayment,
    session: bool,
) -> Result<(), Error> {
    let get = |key: &str| -> Option<&str> {
        let mut values = form
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_str());
        let value = values.next()?;
        if values.next().is_some() {
            None
        } else {
            Some(value)
        }
    };
    let (amount, currency) = if session {
        (
            "line_items[0][price_data][unit_amount]",
            "line_items[0][price_data][currency]",
        )
    } else {
        ("amount", "currency")
    };
    if !expected_valid(expected)
        || get(amount).and_then(|v| v.parse::<i64>().ok()) != Some(expected.amount_grosze)
        || get(currency) != Some("pln")
        || get("metadata[quote_id]") != Some(expected.correlation_id.as_str())
    {
        return Err(Error::InvalidInput);
    }
    if session {
        if get("mode") != Some("payment") || get("line_items[0][quantity]") != Some("1") {
            return Err(Error::InvalidInput);
        }
        for name in ["success_url", "cancel_url"] {
            validate_return_url(get(name).ok_or(Error::InvalidInput)?)?;
        }
    }
    Ok(())
}
/// Return URLs come from server configuration. HTTP is allowed only for localhost.
pub fn validate_return_url(value: &str) -> Result<(), Error> {
    if value.len() > 2048 || value.bytes().any(|b| b.is_ascii_control()) {
        return Err(Error::InvalidInput);
    }
    let url = Url::parse(value).map_err(|_| Error::InvalidInput)?;
    let host = url.host_str().ok_or(Error::InvalidInput)?;
    if !url.username().is_empty()
        || url.password().is_some()
        || !(url.scheme() == "https"
            || (url.scheme() == "http" && matches!(host, "localhost" | "127.0.0.1" | "[::1]")))
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
fn matches(value: &Value, expected: &ExpectedPayment, session: bool) -> bool {
    expected_valid(expected)
        && value["livemode"] == false
        && value["currency"] == "pln"
        && value[if session { "amount_total" } else { "amount" }].as_i64()
            == Some(expected.amount_grosze)
        && value["metadata"]["quote_id"].as_str() == Some(expected.correlation_id.as_str())
}
fn text(value: &Value, key: &str) -> Result<String, Error> {
    value[key]
        .as_str()
        .map(str::to_owned)
        .ok_or(Error::Mismatch)
}
fn checkout(value: Value, expected: &ExpectedPayment, id: Option<&str>) -> Result<Checkout, Error> {
    let actual = text(&value, "id")?;
    if validate_id(&actual, "cs_test_").is_err()
        || id.is_some_and(|v| v != actual)
        || !matches(&value, expected, true)
        || value["mode"] != "payment"
    {
        return Err(Error::Mismatch);
    }
    let status = text(&value, "status")?;
    let payment_status = text(&value, "payment_status")?;
    if !["open", "complete", "expired"].contains(&status.as_str())
        || !["paid", "unpaid", "no_payment_required"].contains(&payment_status.as_str())
    {
        return Err(Error::Mismatch);
    }
    let url = match value.get("url").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => {
            let raw = v.as_str().ok_or(Error::Mismatch)?;
            let parsed = Url::parse(raw).map_err(|_| Error::Mismatch)?;
            if parsed.scheme() != "https"
                || parsed.host_str() != Some("checkout.stripe.com")
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.port().is_some()
            {
                return Err(Error::Mismatch);
            }
            Some(raw.to_owned())
        }
    };
    if status == "open" && url.is_none() {
        return Err(Error::Mismatch);
    }
    let payment_intent = match value.get("payment_intent").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => {
            let id = v.as_str().ok_or(Error::Mismatch)?;
            validate_id(id, "pi_").map_err(|_| Error::Mismatch)?;
            Some(id.to_owned())
        }
    };
    Ok(Checkout {
        object: value,
        id: actual,
        url,
        status,
        payment_status,
        payment_intent,
    })
}
fn intent(value: Value, expected: &ExpectedPayment, id: Option<&str>) -> Result<Intent, Error> {
    let actual = text(&value, "id")?;
    if validate_id(&actual, "pi_").is_err()
        || id.is_some_and(|v| v != actual)
        || !matches(&value, expected, false)
    {
        return Err(Error::Mismatch);
    }
    let status = text(&value, "status")?;
    if ![
        "requires_payment_method",
        "requires_confirmation",
        "requires_action",
        "processing",
        "requires_capture",
        "canceled",
        "succeeded",
    ]
    .contains(&status.as_str())
        || (status == "succeeded"
            && value["amount_received"].as_i64() != Some(expected.amount_grosze))
    {
        return Err(Error::Mismatch);
    }
    let client_secret = match value.get("client_secret").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => Some(
            v.as_str()
                .filter(|s| s.starts_with(&format!("{actual}_secret_")) && s.len() <= 512)
                .ok_or(Error::Mismatch)?
                .to_owned(),
        ),
    };
    Ok(Intent {
        object: value,
        id: actual,
        status,
        client_secret,
    })
}

/// Verify the exact raw body before parsing; accepts rotated v1 signatures, rejects replay age.
pub fn verify_webhook(
    secret: &str,
    signature: &str,
    raw_body: &[u8],
    now: i64,
) -> Result<WebhookEvent, Error> {
    if !secret.starts_with("whsec_")
        || secret.len() > 256
        || raw_body.len() > MAX_WEBHOOK
        || signature.len() > 4096
    {
        return Err(Error::InvalidSignature);
    }
    let mut timestamp = None;
    let mut signatures = Vec::new();
    for part in signature.split(',') {
        let Some((key, value)) = part.trim().split_once('=') else {
            return Err(Error::InvalidSignature);
        };
        match key {
            "t" => {
                if timestamp.is_some()
                    || value.is_empty()
                    || !value.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err(Error::InvalidSignature);
                }
                timestamp = Some((
                    value,
                    value.parse::<i64>().map_err(|_| Error::InvalidSignature)?,
                ));
            }
            "v1" => {
                if signatures.len() >= 16 {
                    return Err(Error::InvalidSignature);
                }
                signatures.push(value);
            }
            _ => {}
        }
    }
    let (raw_time, time) = timestamp.ok_or(Error::InvalidSignature)?;
    if now.abs_diff(time) > 300 {
        return Err(Error::InvalidSignature);
    }
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| Error::InvalidSignature)?;
    mac.update(raw_time.as_bytes());
    mac.update(b".");
    mac.update(raw_body);
    let valid = signatures.into_iter().any(|sig| {
        let mut decoded = [0u8; 32];
        if sig.len() != 64 {
            return false;
        }
        for (i, pair) in sig.as_bytes().chunks_exact(2).enumerate() {
            let Some(a) = (pair[0] as char).to_digit(16) else {
                return false;
            };
            let Some(b) = (pair[1] as char).to_digit(16) else {
                return false;
            };
            decoded[i] = ((a << 4) | b) as u8;
        }
        mac.clone().verify_slice(&decoded).is_ok()
    });
    if !valid {
        return Err(Error::InvalidSignature);
    }
    let value: Value = serde_json::from_slice(raw_body).map_err(|_| Error::InvalidInput)?;
    let id = text(&value, "id")?;
    let event_type = text(&value, "type")?;
    let object_id = text(&value["data"]["object"], "id")?;
    if validate_id(&id, "evt_").is_err()
        || event_type.len() > 100
        || event_type.is_empty()
        || !event_type
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'.' || b == b'_')
        || value["livemode"] != false
        || value["data"]["object"]["livemode"] != false
    {
        return Err(Error::Mismatch);
    }
    let prefix = if event_type.starts_with("checkout.session.") {
        "cs_test_"
    } else if event_type.starts_with("payment_intent.") {
        "pi_"
    } else {
        return Err(Error::Mismatch);
    };
    validate_id(&object_id, prefix).map_err(|_| Error::Mismatch)?;
    Ok(WebhookEvent {
        id,
        event_type,
        object_id,
        livemode: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn expected() -> ExpectedPayment {
        ExpectedPayment {
            correlation_id: "order-123".into(),
            amount_grosze: 4200,
        }
    }
    fn session() -> Value {
        json!({"id":"cs_test_example","mode":"payment","status":"complete","payment_status":"paid","amount_total":4200,"currency":"pln","livemode":false,"metadata":{"quote_id":"order-123"},"payment_intent":"pi_example","url":null})
    }
    #[test]
    fn payment_requires_exact_server_correlation_and_positive_received_amount() {
        let original = session();
        assert_eq!(
            checkout(original.clone(), &expected(), None)
                .unwrap()
                .object,
            original
        );
        assert!(
            checkout(session(), &expected(), Some("cs_test_example"))
                .unwrap()
                .paid()
        );
        for (key, value) in [
            ("amount_total", json!(4199)),
            ("currency", json!("usd")),
            ("livemode", json!(true)),
            ("mode", json!("subscription")),
            ("id", json!("cs_test_other")),
            ("metadata", json!({"quote_id":"other"})),
        ] {
            let mut bad = session();
            bad[key] = value;
            assert!(
                matches!(
                    checkout(bad, &expected(), Some("cs_test_example")),
                    Err(Error::Mismatch)
                ),
                "{key}"
            );
        }
        let mut expired = session();
        expired["status"] = json!("expired");
        expired["payment_status"] = json!("unpaid");
        assert!(!checkout(expired, &expected(), None).unwrap().paid());
        let mut value = json!({"id":"pi_example","amount":4200,"amount_received":4199,"currency":"pln","livemode":false,"metadata":{"quote_id":"order-123"},"status":"succeeded","client_secret":null});
        assert!(matches!(
            intent(value.clone(), &expected(), None),
            Err(Error::Mismatch)
        ));
        value["amount_received"] = json!(4200);
        assert!(
            intent(value, &expected(), Some("pi_example"))
                .unwrap()
                .paid()
        );
    }
    #[test]
    fn redirect_and_provider_ids_cannot_escape_expected_hosts() {
        for value in [
            "http://attacker.example/x",
            "https://user:password@host.example/",
            "file:///tmp/x",
            "javascript:alert(1)",
            "http://localhost.attacker.example/",
        ] {
            assert!(validate_return_url(value).is_err(), "{value}");
        }
        for value in [
            "https://rapidmaker.pl/done",
            "http://localhost:18430/zamowienie?session={CHECKOUT_SESSION_ID}",
            "http://127.0.0.1:18430/cancel",
        ] {
            assert!(validate_return_url(value).is_ok(), "{value}");
        }
        assert!(validate_id("pi_example/../other", "pi_").is_err());
        let mut bad = session();
        bad["url"] = json!("https://checkout.stripe.com.attacker.example/c/pay/x");
        assert!(matches!(
            checkout(bad, &expected(), None),
            Err(Error::Mismatch)
        ));
    }
    fn signature(body: &[u8], time: i64) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(b"whsec_test_secret").unwrap();
        mac.update(format!("{time}.").as_bytes());
        mac.update(body);
        format!(
            "t={time},v1={}",
            mac.finalize()
                .into_bytes()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        )
    }
    #[test]
    fn webhook_checks_raw_bytes_timestamp_rotated_signatures_and_test_mode() {
        let now = 1700000000;
        let body=br#"{"id":"evt_example","type":"checkout.session.completed","livemode":false,"data":{"object":{"id":"cs_test_example","livemode":false}}}"#;
        let good = signature(body, now);
        let event = verify_webhook("whsec_test_secret", &good, body, now).unwrap();
        assert_eq!(event.object_id, "cs_test_example");
        assert!(
            verify_webhook(
                "whsec_test_secret",
                &format!("{good},v1={}", "00".repeat(32)),
                body,
                now
            )
            .is_ok()
        );
        assert!(
            verify_webhook(
                "whsec_test_secret",
                &format!(
                    "t={now},v1={},{}",
                    "00".repeat(32),
                    good.split_once(',').unwrap().1
                ),
                body,
                now
            )
            .is_ok()
        );
        for delta in [-301, 301] {
            assert_eq!(
                verify_webhook(
                    "whsec_test_secret",
                    &signature(body, now + delta),
                    body,
                    now
                )
                .unwrap_err(),
                Error::InvalidSignature
            );
        }
        assert!(verify_webhook("whsec_test_secret", &good, body, now + 300).is_ok());
        assert_eq!(
            verify_webhook("whsec_test_secret", &good, b"{}", now).unwrap_err(),
            Error::InvalidSignature
        );
        assert_eq!(
            verify_webhook("whsec_test_secret", &format!("{good},t={now}"), body, now).unwrap_err(),
            Error::InvalidSignature
        );
        assert!(
            verify_webhook(
                "whsec_test_secret",
                "t=999999999999999999999999,v1=aa",
                body,
                now
            )
            .is_err()
        );
        let live = String::from_utf8(body.to_vec())
            .unwrap()
            .replace("false", "true");
        assert_eq!(
            verify_webhook(
                "whsec_test_secret",
                &signature(live.as_bytes(), now),
                live.as_bytes(),
                now
            )
            .unwrap_err(),
            Error::Mismatch
        );
    }
}
