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
    },
}

impl Notification<'_> {
    pub(crate) fn render(self) -> Result<(&'static str, String), MailError> {
        let field = |v: &str| {
            if v.is_empty() || v.len() > 160 || v.chars().any(char::is_control) {
                Err(MailError::InvalidMessage)
            } else {
                Ok(())
            }
        };
        Ok(match self {
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
            Self::ChatMessage { reference } => {
                field(reference)?;
                ("Rapidmaker — nowa wiadomość", format!("Masz nową wiadomość dotyczącą zamówienia {reference}.\n\nZaloguj się do panelu: https://rapidmaker.pl/konto/zamowienia"))
            }
        })
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
    },
}

impl EmailPayload {
    pub fn validate(&self) -> Result<(), MailError> {
        if self.version != 1 {
            return Err(MailError::InvalidMessage);
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
            Self::ChatMessage { reference } => Notification::ChatMessage { reference },
        }
    }
}
