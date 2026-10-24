use crate::MailError;

pub enum Notification<'a> {
    Test,
    LoginCode(&'a str),
    OrderConfirmed {
        reference: &'a str,
    },
    PaymentReceived {
        reference: &'a str,
        amount_grosze: u64,
    },
    StatusChanged {
        reference: &'a str,
        status: &'a str,
    },
    ChatMessage {
        reference: &'a str,
        body: &'a str,
        operator: bool,
    },
}

impl Notification<'_> {
    pub(crate) fn render(self) -> Result<(String, String), MailError> {
        let field = |v: &str| {
            if v.is_empty() || v.len() > 160 || v.chars().any(char::is_control) {
                Err(MailError::InvalidMessage)
            } else {
                Ok(())
            }
        };
        let chat_reference = match &self {
            Self::ChatMessage { reference, .. } => Some(*reference),
            _ => None,
        };
        let (subject, body) = match self {
            Self::Test => ("Rapidmaker — test email", "Wysyłka testowa z backendu Rust Rapidmaker. Ta wiadomość nie zawiera kodu logowania ani danych zamówienia.".into()),
            Self::LoginCode(code) => {
                if code.len() != 8 || !code.bytes().all(|v| v.is_ascii_digit()) {
                    return Err(MailError::InvalidMessage);
                }
                ("Rapidmaker — kod logowania", format!("Twój kod logowania: {code}\n\nKod jest jednorazowy i ważny przez 10 minut. Jeśli nie prosisz o logowanie, zignoruj tę wiadomość."))
            }
            Self::OrderConfirmed { reference } => {
                field(reference)?;
                ("Rapidmaker — zamówienie przyjęte", format!("Przyjęliśmy zamówienie {reference}. Potwierdzenie przyjęcia nie oznacza potwierdzenia płatności."))
            }
            Self::PaymentReceived { reference, amount_grosze } => {
                field(reference)?;
                ("Rapidmaker — płatność otrzymana", format!("Otrzymaliśmy płatność {}.{:02} PLN za zamówienie {reference}.", amount_grosze / 100, amount_grosze % 100))
            }
            Self::StatusChanged { reference, status } => {
                field(reference)?;
                field(status)?;
                ("Rapidmaker — status zamówienia", format!("Zamówienie {reference}\nNowy status: {status}"))
            }
            Self::ChatMessage { reference, body, operator } => {
                field(reference)?;
                if !reference.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') { return Err(MailError::InvalidMessage); }
                if body.len() > 24000 || body.chars().any(|c| c.is_control() && c != '\n' && c != '\t') { return Err(MailError::InvalidMessage); }
                let origin = std::env::var("AUTH_ORIGIN").unwrap_or_else(|_| "http://localhost:18430".into());
                if origin.len() > 512 || !origin.starts_with("https://") && !origin.starts_with("http://localhost:") && !origin.starts_with("http://127.0.0.1:") || origin.chars().any(char::is_control) || origin.contains(['?', '#', '@']) { return Err(MailError::Configuration); }
                let panel = if operator { "admin/zamowienia" } else { "konto/zamowienia" };
                let reply = if body.is_empty() { "Odpowiedz w panelu." } else { "Odpowiedz na ten email lub w panelu." };
                ("Rapidmaker — nowa wiadomość", format!("Zamówienie {reference}\n\n{body}\n\nRozmowa: {}/{panel}/{reference}\n{reply}", origin.trim_end_matches('/')))
            }
        };
        Ok((
            chat_reference
                .map(|id| format!("Rapidmaker — zamówienie {id}"))
                .unwrap_or_else(|| subject.to_owned()),
            body,
        ))
    }
}

/// Versioned, bounded snapshots; login codes must never enter the durable outbox.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmailPayload {
    pub version: u8,
    pub notification: EmailNotification,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EmailNotification {
    OrderConfirmed {
        reference: String,
    },
    PaymentReceived {
        reference: String,
        amount_grosze: u64,
    },
    StatusChanged {
        reference: String,
        status: String,
    },
    ChatMessage {
        reference: String,
        #[serde(default)]
        body: String,
        #[serde(default)]
        operator: bool,
        #[serde(default)]
        reply_route: Option<String>,
        #[serde(default)]
        first_sequence: i64,
        #[serde(default)]
        last_sequence: i64,
    },
}

impl EmailPayload {
    pub fn validate(&self) -> Result<(), MailError> {
        if self.version != 1 {
            return Err(MailError::InvalidMessage);
        }
        if let EmailNotification::ChatMessage {
            reference,
            body,
            reply_route,
            first_sequence,
            last_sequence,
            ..
        } = &self.notification
        {
            if reference.is_empty()
                || reference.len() > 160
                || reference.chars().any(char::is_control)
                || body.len() > 24000
                || body
                    .chars()
                    .any(|c| c.is_control() && c != '\n' && c != '\t')
                || *first_sequence < 0
                || *last_sequence < *first_sequence
                || reply_route.as_ref().is_some_and(|v| v.len() != 36)
            {
                return Err(MailError::InvalidMessage);
            }
            return Ok(());
        }
        self.notification.borrowed().render().map(|_| ())
    }
}

impl EmailNotification {
    pub(crate) fn borrowed(&self) -> Notification<'_> {
        match self {
            Self::OrderConfirmed { reference } => Notification::OrderConfirmed { reference },
            Self::PaymentReceived {
                reference,
                amount_grosze,
            } => Notification::PaymentReceived {
                reference,
                amount_grosze: *amount_grosze,
            },
            Self::StatusChanged { reference, status } => {
                Notification::StatusChanged { reference, status }
            }
            Self::ChatMessage {
                reference,
                body,
                operator,
                ..
            } => Notification::ChatMessage {
                reference,
                body,
                operator: *operator,
            },
        }
    }
}
