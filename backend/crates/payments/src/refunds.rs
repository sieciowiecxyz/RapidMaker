use super::{Error, Method, Stripe, text, validate_id};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct ExpectedRefund {
    pub refund_id: String,
    pub order_id: String,
    pub payment_intent: String,
    pub amount_grosze: i64,
}

#[derive(Debug)]
pub struct Refund {
    pub id: String,
    pub status: String,
    pub amount_grosze: i64,
    pub payment_intent: String,
}

impl Stripe {
    /// Persist the request/key and verify the original paid PaymentIntent before calling.
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn create_refund(
        &self,
        expected: &ExpectedRefund,
        idempotency_key: &str,
    ) -> Result<Refund, Error> {
        validate_expected(expected)?;
        let form = [
            ("payment_intent".into(), expected.payment_intent.clone()),
            ("amount".into(), expected.amount_grosze.to_string()),
            ("reason".into(), "requested_by_customer".into()),
            ("metadata[refund_id]".into(), expected.refund_id.clone()),
            ("metadata[order_id]".into(), expected.order_id.clone()),
        ];
        let value = self
            .request(Method::POST, "refunds", Some((&form, idempotency_key)))
            .await?;
        refund(value, expected, None)
    }

    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn retrieve_refund(
        &self,
        provider_id: &str,
        expected: &ExpectedRefund,
    ) -> Result<Refund, Error> {
        validate_expected(expected)?;
        validate_id(provider_id, "re_")?;
        let value = self
            .request(Method::GET, &format!("refunds/{provider_id}"), None)
            .await?;
        refund(value, expected, Some(provider_id))
    }
}

fn validate_expected(expected: &ExpectedRefund) -> Result<(), Error> {
    if !(1..=99_999_999).contains(&expected.amount_grosze)
        || [&expected.refund_id, &expected.order_id].iter().any(|id| {
            id.is_empty()
                || id.len() > 128
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        })
    {
        return Err(Error::InvalidInput);
    }
    validate_id(&expected.payment_intent, "pi_")
}

fn refund(value: Value, expected: &ExpectedRefund, id: Option<&str>) -> Result<Refund, Error> {
    validate_expected(expected)?;
    let actual = text(&value, "id")?;
    // Stripe Refund has no livemode field. The transport accepts test keys only;
    // the caller verifies livemode=false on the original paid PaymentIntent.
    if validate_id(&actual, "re_").is_err()
        || id.is_some_and(|id| id != actual)
        || value["object"] != "refund"
        || value.get("livemode").is_some_and(|mode| mode != false)
        || value["currency"] != "pln"
        || value["amount"].as_i64() != Some(expected.amount_grosze)
        || value["payment_intent"].as_str() != Some(expected.payment_intent.as_str())
        || value["metadata"]["refund_id"].as_str() != Some(expected.refund_id.as_str())
        || value["metadata"]["order_id"].as_str() != Some(expected.order_id.as_str())
    {
        return Err(Error::Mismatch);
    }
    let status = text(&value, "status")?;
    if ![
        "pending",
        "requires_action",
        "succeeded",
        "failed",
        "canceled",
    ]
    .contains(&status.as_str())
    {
        return Err(Error::Mismatch);
    }
    Ok(Refund {
        id: actual,
        status,
        amount_grosze: expected.amount_grosze,
        payment_intent: expected.payment_intent.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn expected() -> ExpectedRefund {
        ExpectedRefund {
            refund_id: "refund-123".into(),
            order_id: "order-123".into(),
            payment_intent: "pi_example".into(),
            amount_grosze: 1200,
        }
    }

    fn response() -> Value {
        json!({"id":"re_example","object":"refund","amount":1200,"currency":"pln",
            "payment_intent":"pi_example","metadata":{"refund_id":"refund-123","order_id":"order-123"},"status":"succeeded"})
    }

    #[test]
    fn provider_refund_requires_exact_correlation_and_known_status() {
        for status in [
            "pending",
            "requires_action",
            "succeeded",
            "failed",
            "canceled",
        ] {
            let mut value = response();
            value["status"] = json!(status);
            let validated = refund(value, &expected(), Some("re_example")).unwrap();
            assert_eq!(validated.status, status);
            assert_eq!(validated.amount_grosze, 1200);
            assert_eq!(validated.payment_intent, "pi_example");
        }
        for (key, value) in [
            ("id", json!("pi_example")),
            ("object", json!("payment_intent")),
            ("amount", json!(1201)),
            ("currency", json!("eur")),
            ("payment_intent", json!("pi_other")),
            (
                "metadata",
                json!({"refund_id":"other","order_id":"order-123"}),
            ),
            (
                "metadata",
                json!({"refund_id":"refund-123","order_id":"other"}),
            ),
            ("livemode", json!(true)),
            ("livemode", Value::Null),
            ("status", json!("unknown")),
            ("status", Value::Null),
        ] {
            let mut changed = response();
            changed[key] = value;
            assert_eq!(
                refund(changed, &expected(), None).unwrap_err(),
                Error::Mismatch,
                "{key}"
            );
        }
        assert_eq!(
            refund(response(), &expected(), Some("re_other")).unwrap_err(),
            Error::Mismatch
        );
    }

    #[test]
    fn expected_refund_rejects_invalid_ids_and_amounts_before_network() {
        for amount in [i64::MIN, -1, 0, 100_000_000, i64::MAX] {
            let mut expected = expected();
            expected.amount_grosze = amount;
            assert_eq!(validate_expected(&expected), Err(Error::InvalidInput));
        }
        for id in ["", "bad/id", "bad\nheader", "ą", &"x".repeat(129)] {
            let mut expected = expected();
            expected.refund_id = id.into();
            assert_eq!(validate_expected(&expected), Err(Error::InvalidInput));
            expected.refund_id = "valid-id".into();
            expected.order_id = id.into();
            assert_eq!(validate_expected(&expected), Err(Error::InvalidInput));
        }
        let mut expected = expected();
        expected.payment_intent = "pi_example/../other".into();
        assert_eq!(validate_expected(&expected), Err(Error::InvalidInput));
    }
}
