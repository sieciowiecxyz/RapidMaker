use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Recipient {
    pub name: String,
    pub email: String,
    pub phone: String,
    #[serde(default = "poland")]
    pub country: String,
    pub street: String,
    pub apartment: String,
    pub postal_code: String,
    pub city: String,
    pub invoice: Option<Invoice>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Invoice {
    pub company: String,
    pub nip: String,
    pub street: String,
    pub postal_code: String,
    pub city: String,
    #[serde(default = "poland")]
    pub country: String,
}
fn poland() -> String {
    "PL".into()
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    #[default]
    Courier,
    Locker,
    Pickup,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct DeliveryChoice {
    pub method: Method,
    /// Pass the selected Point.code unchanged; the backend verifies its network and address.
    pub point_id: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Validation {
    pub field_errors: BTreeMap<String, String>,
}
impl Validation {
    pub(crate) fn field(field: &str, message: &str) -> Self {
        Self {
            field_errors: [(field.into(), message.into())].into(),
        }
    }
}
fn text(
    value: &mut String,
    name: &str,
    required: bool,
    max: usize,
    errors: &mut BTreeMap<String, String>,
) {
    let invalid = value.len() > max || value.chars().any(char::is_control);
    if !invalid {
        *value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    if invalid || (required && value.is_empty()) {
        errors.insert(name.into(), "Uzupełnij poprawnie to pole.".into());
    }
}
fn postcode(
    value: &mut String,
    field: &str,
    required: bool,
    errors: &mut BTreeMap<String, String>,
) {
    text(value, field, required, 16, errors);
    if value.is_empty() {
        return;
    }
    let digits: String = value.chars().filter(|c| *c != '-' && *c != ' ').collect();
    if digits.len() == 5 && digits.bytes().all(|c| c.is_ascii_digit()) {
        *value = format!("{}-{}", &digits[..2], &digits[2..]);
    } else {
        errors.insert(field.into(), "Podaj kod pocztowy w formacie 00-000.".into());
    }
}
pub(crate) const POINT_CARRIERS: &[&str] = &["inpost", "orlen", "dpd", "dhl"];
pub(crate) fn raw_point_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
pub(crate) fn point_parts(id: &str) -> Option<(&str, &str)> {
    let (carrier, raw) = id.split_once(':').unwrap_or(("inpost", id));
    (POINT_CARRIERS.contains(&carrier) && raw_point_id(raw)).then_some((carrier, raw))
}
pub fn valid_point_id(id: &str) -> bool {
    point_parts(id).is_some()
}
pub(crate) fn service_matches(choice: &DeliveryChoice, carrier: &str) -> bool {
    match choice.method {
        Method::Locker => choice
            .point_id
            .as_deref()
            .and_then(point_parts)
            .is_some_and(|(network, _)| network == carrier),
        Method::Courier => ["dpd", "dhl", "ups", "gls", "fedex", "inpost"].contains(&carrier),
        Method::Pickup => false,
    }
}
impl Recipient {
    /// Drafts may omit required values; malformed supplied values are still errors.
    pub fn normalize_draft(
        &mut self,
        delivery: &DeliveryChoice,
    ) -> Result<Vec<String>, Validation> {
        let Err(mut validation) = self.normalize(delivery) else {
            return Ok(vec![]);
        };
        let mut missing = vec![];
        validation.field_errors.retain(|field, _| {
            let empty = match field.as_str() {
                "name" => self.name.is_empty(),
                "email" => self.email.is_empty(),
                "phone" => self.phone.is_empty(),
                "street" => self.street.is_empty(),
                "postal_code" => self.postal_code.is_empty(),
                "city" => self.city.is_empty(),
                "delivery.point_id" => {
                    delivery.method == Method::Locker
                        && delivery.point_id.as_deref().is_none_or(str::is_empty)
                }
                "invoice.company" => self.invoice.as_ref().is_some_and(|v| v.company.is_empty()),
                "invoice.nip" => self.invoice.as_ref().is_some_and(|v| v.nip.is_empty()),
                "invoice.street" => self.invoice.as_ref().is_some_and(|v| v.street.is_empty()),
                "invoice.postal_code" => self
                    .invoice
                    .as_ref()
                    .is_some_and(|v| v.postal_code.is_empty()),
                "invoice.city" => self.invoice.as_ref().is_some_and(|v| v.city.is_empty()),
                _ => false,
            };
            if empty {
                missing.push(field.clone());
            }
            !empty
        });
        if validation.field_errors.is_empty() {
            Ok(missing)
        } else {
            Err(validation)
        }
    }

    /// Syntax validation only; the carrier still verifies serviceability.
    pub fn normalize(&mut self, delivery: &DeliveryChoice) -> Result<(), Validation> {
        let mut errors = BTreeMap::new();
        text(&mut self.name, "name", true, 120, &mut errors);
        text(&mut self.email, "email", true, 254, &mut errors);
        let email: Vec<_> = self.email.split('@').collect();
        if email.len() != 2
            || email[0].is_empty()
            || email[1].is_empty()
            || !email[1].contains('.')
            || !self.email.is_ascii()
            || self.email.chars().any(char::is_whitespace)
        {
            errors.insert("email".into(), "Podaj poprawny adres email.".into());
        }
        text(&mut self.phone, "phone", true, 40, &mut errors);
        let phone: String = self
            .phone
            .chars()
            .filter(|c| !matches!(c, ' ' | '-' | '(' | ')'))
            .collect();
        let national = phone
            .strip_prefix("+48")
            .or_else(|| phone.strip_prefix("0048"))
            .unwrap_or(&phone);
        if national.len() == 9
            && national.bytes().all(|c| c.is_ascii_digit())
            && !national.starts_with('0')
        {
            self.phone = format!("+48{national}");
        } else {
            errors.insert(
                "phone".into(),
                "Podaj polski numer telefonu: 9 cyfr lub +48 i 9 cyfr.".into(),
            );
        }
        if self.country.is_empty() {
            self.country = poland();
        }
        text(&mut self.country, "country", true, 2, &mut errors);
        self.country.make_ascii_uppercase();
        if self.country != "PL" {
            errors.insert(
                "country".into(),
                "Obsługujemy dostawę tylko w Polsce.".into(),
            );
        }
        let courier = delivery.method == Method::Courier;
        text(&mut self.street, "street", courier, 160, &mut errors);
        if courier && !self.street.chars().any(|c| c.is_ascii_digit()) {
            errors.insert("street".into(), "Dodaj numer budynku do ulicy.".into());
        }
        text(&mut self.apartment, "apartment", false, 30, &mut errors);
        postcode(&mut self.postal_code, "postal_code", courier, &mut errors);
        text(&mut self.city, "city", courier, 100, &mut errors);
        if delivery.method == Method::Locker
            && !delivery.point_id.as_deref().is_some_and(valid_point_id)
        {
            errors.insert(
                "delivery.point_id".into(),
                "Wybierz dostępny punkt odbioru.".into(),
            );
        }
        if delivery.method != Method::Locker && delivery.point_id.is_some() {
            errors.insert(
                "delivery.point_id".into(),
                "Punkt odbioru pasuje tylko do dostawy do punktu.".into(),
            );
        }
        if let Some(invoice) = self.invoice.as_mut() {
            text(
                &mut invoice.company,
                "invoice.company",
                true,
                160,
                &mut errors,
            );
            text(&mut invoice.nip, "invoice.nip", true, 20, &mut errors);
            invoice.nip = invoice
                .nip
                .chars()
                .filter(|c| !matches!(c, ' ' | '-'))
                .collect();
            let digits: Vec<u32> = invoice
                .nip
                .chars()
                .filter_map(|c| c.to_digit(10).filter(|_| c.is_ascii()))
                .collect();
            let checksum = digits
                .iter()
                .zip([6, 5, 7, 2, 3, 4, 5, 6, 7])
                .map(|(d, w)| d * w)
                .sum::<u32>()
                % 11;
            if digits.len() != 10
                || invoice.nip.len() != 10
                || !digits.iter().any(|d| *d != 0)
                || checksum == 10
                || digits[9] != checksum
            {
                errors.insert("invoice.nip".into(), "Podaj poprawny polski NIP.".into());
            }
            text(
                &mut invoice.street,
                "invoice.street",
                true,
                160,
                &mut errors,
            );
            postcode(
                &mut invoice.postal_code,
                "invoice.postal_code",
                true,
                &mut errors,
            );
            text(&mut invoice.city, "invoice.city", true, 100, &mut errors);
            if invoice.country.is_empty() {
                invoice.country = poland();
            }
            text(
                &mut invoice.country,
                "invoice.country",
                true,
                2,
                &mut errors,
            );
            invoice.country.make_ascii_uppercase();
            if invoice.country != "PL" {
                errors.insert(
                    "invoice.country".into(),
                    "Obsługujemy dane faktury w Polsce.".into(),
                );
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Validation {
                field_errors: errors,
            })
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn point_ids_preserve_inpost_and_reject_cross_network_services() {
        assert_eq!(point_parts("PIA01M"), Some(("inpost", "PIA01M")));
        assert_eq!(point_parts("inpost:PIA01M"), point_parts("PIA01M"));
        for network in POINT_CARRIERS {
            let id = format!("{network}:ABC-1_2");
            assert!(valid_point_id(&id));
            let choice = DeliveryChoice {
                method: Method::Locker,
                point_id: Some(id),
            };
            for candidate in POINT_CARRIERS {
                assert_eq!(service_matches(&choice, candidate), candidate == network);
            }
        }
        for id in [
            "",
            "unknown:ABC",
            "dpd:",
            "dpd:inpost:ABC",
            "dpd:../ABC",
            "orlen:ABC/DEF",
        ] {
            assert!(!valid_point_id(id));
        }
        assert!(!valid_point_id(&format!("dpd:{}", "A".repeat(65))));
        assert!(service_matches(&DeliveryChoice::default(), "inpost"));
        assert!(!service_matches(&DeliveryChoice::default(), "orlen"));
    }
    fn recipient() -> Recipient {
        Recipient {
            name: " Jan   Kowalski ".into(),
            email: "jan@example.org".into(),
            phone: "(123) 456-789".into(),
            country: "pl".into(),
            street: "Długa 2".into(),
            postal_code: "00 001".into(),
            city: "Warszawa".into(),
            ..Default::default()
        }
    }
    #[test]
    fn normalization_and_method_requirements() {
        let mut r = recipient();
        let courier = DeliveryChoice {
            method: Method::Courier,
            point_id: None,
        };
        r.normalize(&courier).unwrap();
        assert_eq!(
            (r.name.as_str(), r.phone.as_str(), r.postal_code.as_str()),
            ("Jan Kowalski", "+48123456789", "00-001")
        );
        r.street.clear();
        r.city.clear();
        r.postal_code.clear();
        assert!(
            r.normalize(&DeliveryChoice {
                method: Method::Pickup,
                point_id: None
            })
            .is_ok()
        );
        assert!(
            r.normalize(&courier)
                .unwrap_err()
                .field_errors
                .contains_key("postal_code")
        );
        assert!(
            r.normalize(&DeliveryChoice {
                method: Method::Locker,
                point_id: None
            })
            .unwrap_err()
            .field_errors
            .contains_key("delivery.point_id")
        );
    }
    #[test]
    fn incomplete_draft_separates_missing_fields_from_malformed_values() {
        let mut draft: Recipient = serde_json::from_str("{}").unwrap();
        let choice: DeliveryChoice = serde_json::from_str("{}").unwrap();
        let missing = draft.normalize_draft(&choice).unwrap();
        assert_eq!(draft.country, "PL");
        assert!(missing.contains(&"name".into()) && missing.contains(&"postal_code".into()));
        draft.email = "broken".into();
        assert!(
            draft
                .normalize_draft(&choice)
                .unwrap_err()
                .field_errors
                .contains_key("email")
        );
        let mut complete = recipient();
        complete.invoice = Some(Invoice {
            company: "Firma".into(),
            ..Default::default()
        });
        let missing = complete.normalize_draft(&choice).unwrap();
        assert!(missing.contains(&"invoice.nip".into()));
        assert!(complete.normalize(&choice).is_err());
    }
    #[test]
    fn rejects_unknown_fields_control_characters_and_bad_invoice() {
        assert!(serde_json::from_str::<Recipient>(r#"{"unexpected":true}"#).is_err());
        let mut r = recipient();
        r.name.push('\n');
        r.email = "a@@b.pl".into();
        r.invoice = Some(Invoice {
            company: "Firma".into(),
            nip: "123".into(),
            street: "A 1".into(),
            postal_code: "00000".into(),
            city: "Warszawa".into(),
            country: "PL".into(),
        });
        let errors = r
            .normalize(&DeliveryChoice {
                method: Method::Pickup,
                point_id: None,
            })
            .unwrap_err()
            .field_errors;
        assert!(
            errors.contains_key("name")
                && errors.contains_key("email")
                && errors.contains_key("invoice.nip")
        );
    }
}
