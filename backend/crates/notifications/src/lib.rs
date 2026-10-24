//! Small SMTP sender. Callers own authorization, rate limits and delivery policy.
mod inbound;
mod messages;
mod replies;
pub use inbound::{Imap, ImapBatch, ImapCursor, InboundDisposition, InboundMail};
pub use messages::{EmailNotification, EmailPayload, Notification};
pub use replies::{parse_inbound, reply_reference, verify_reply_reference};

use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, header::ContentType},
    transport::smtp::{self, authentication::Credentials},
};
use std::{fmt, time::Duration};
use tokio::{
    sync::Semaphore,
    time::{Instant, timeout_at},
};

const DEADLINE: Duration = Duration::from_secs(20);

#[derive(Debug, PartialEq, Eq)]
pub enum MailError {
    Disabled,
    Configuration,
    InvalidAddress,
    InvalidMessage,
    TestRecipient,
    Busy,
    Authentication,
    Transient,
    Rejected,
    Uncertain,
}

impl fmt::Display for MailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Provider responses can contain addresses or credentials; expose only a category.
        write!(f, "mail: {self:?}")
    }
}
impl std::error::Error for MailError {}

pub struct Mailer {
    enabled: Option<Sender>,
    slots: Semaphore,
}

struct Sender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
    reply_to: Mailbox,
    test_to: Option<Mailbox>,
    test_mode: bool,
    reply_key: Option<String>,
}

impl Mailer {
    pub fn enabled(&self) -> bool {
        self.enabled.is_some()
    }

    pub fn from_env() -> Result<Self, MailError> {
        Self::from_values(|key| match std::env::var(key) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(_) => Err(MailError::Configuration),
        })
    }

    fn from_values(
        get: impl Fn(&str) -> Result<Option<String>, MailError>,
    ) -> Result<Self, MailError> {
        let mode = get("MAIL_MODE")?.unwrap_or_else(|| "disabled".into());
        let required = |key| {
            get(key)?
                .filter(|v| !v.trim().is_empty())
                .ok_or(MailError::Configuration)
        };
        let enabled = match mode.as_str() {
            "disabled" => None,
            "test" | "live" => {
                let host = required("MAIL_SMTP_HOST")?;
                let port = required("MAIL_SMTP_PORT")?
                    .parse::<u16>()
                    .map_err(|_| MailError::Configuration)?;
                // This integration is explicitly iCloud: never downgrade to plaintext SMTP.
                if host != "smtp.mail.me.com" || port != 587 {
                    return Err(MailError::Configuration);
                }
                let from = address(&required("MAIL_FROM")?)?;
                let reply_to = address(&required("MAIL_REPLY_TO")?)?;
                let test_to = get("MAIL_TEST_TO")?.map(|v| address(&v)).transpose()?;
                if mode == "test"
                    && test_to
                        .as_ref()
                        .is_none_or(|v| v.email.to_string() != "void@sieciowiec.xyz")
                {
                    return Err(MailError::Configuration);
                }
                let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)
                    .map_err(|_| MailError::Configuration)?
                    .port(port)
                    .credentials(Credentials::new(
                        required("MAIL_USER")?,
                        required("MAIL_PASSWORD")?,
                    ))
                    .timeout(Some(DEADLINE))
                    .build();
                Some(Sender {
                    transport,
                    from,
                    reply_to,
                    test_to,
                    test_mode: mode == "test",
                    reply_key: get("MAIL_REPLY_KEY")?.filter(|v| v.len() >= 32),
                })
            }
            _ => return Err(MailError::Configuration),
        };
        Ok(Self {
            enabled,
            slots: Semaphore::new(2),
        })
    }

    /// Success means SMTP acceptance, not delivery. This function never retries.
    pub async fn send(
        &self,
        recipient: &str,
        notification: Notification<'_>,
    ) -> Result<(), MailError> {
        self.send_inner(recipient, notification, None).await
    }

    pub async fn send_queued(
        &self,
        recipient: &str,
        payload: &EmailPayload,
        id: &str,
    ) -> Result<(), MailError> {
        payload.validate()?;
        if id.is_empty()
            || id.len() > 64
            || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(MailError::InvalidMessage);
        }
        self.send_inner(
            recipient,
            payload.notification.borrowed(),
            Some(match &payload.notification {
                EmailNotification::ChatMessage {
                    reply_route: Some(route),
                    ..
                } => {
                    let sender = self.enabled.as_ref().ok_or(MailError::Disabled)?;
                    reply_reference(
                        route,
                        sender
                            .reply_key
                            .as_deref()
                            .ok_or(MailError::Configuration)?,
                    )?
                }
                _ => format!("<{id}@rapidmaker.pl>"),
            }),
        )
        .await
    }

    async fn send_inner(
        &self,
        recipient: &str,
        notification: Notification<'_>,
        message_id: Option<String>,
    ) -> Result<(), MailError> {
        let deadline = Instant::now() + DEADLINE;
        let sender = self.enabled.as_ref().ok_or(MailError::Disabled)?;
        let message = sender.message_with_id(recipient, notification, message_id)?;
        let _slot = timeout_at(deadline, self.slots.acquire())
            .await
            .map_err(|_| MailError::Busy)?
            .map_err(|_| MailError::Busy)?;
        timeout_at(deadline, sender.transport.send(message))
            .await
            .map_err(|_| MailError::Uncertain)?
            .map_err(smtp_error)?;
        Ok(())
    }

    /// A local diagnostic, restricted to the configured test recipient in test mode.
    pub async fn send_test(&self) -> Result<(), MailError> {
        let sender = self.enabled.as_ref().ok_or(MailError::Disabled)?;
        if !sender.test_mode {
            return Err(MailError::TestRecipient);
        }
        let to = sender.test_to.as_ref().ok_or(MailError::Configuration)?;
        self.send(to.email.as_ref(), Notification::Test).await
    }
}

impl Sender {
    #[cfg(test)]
    fn message(
        &self,
        recipient: &str,
        notification: Notification<'_>,
    ) -> Result<Message, MailError> {
        self.message_with_id(recipient, notification, None)
    }

    fn message_with_id(
        &self,
        recipient: &str,
        notification: Notification<'_>,
        message_id: Option<String>,
    ) -> Result<Message, MailError> {
        let to = address(recipient)?;
        if self.test_mode && self.test_to.as_ref().map(|v| &v.email) != Some(&to.email) {
            return Err(MailError::TestRecipient);
        }
        let (subject, body) = notification.render()?;
        if body.len() > 32 * 1024 {
            return Err(MailError::InvalidMessage);
        }
        Message::builder()
            .from(self.from.clone())
            .reply_to(self.reply_to.clone())
            .to(to)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .message_id(message_id)
            .body(body)
            .map_err(|_| MailError::InvalidMessage)
    }
}

pub fn valid_address(value: &str) -> bool {
    address(value).is_ok()
}

fn address(value: &str) -> Result<Mailbox, MailError> {
    if value.is_empty() || value.len() > 254 || value.chars().any(char::is_control) {
        return Err(MailError::InvalidAddress);
    }
    // Parse an address, not a display name or a comma-separated recipient list.
    Ok(Mailbox::new(
        None,
        value.parse().map_err(|_| MailError::InvalidAddress)?,
    ))
}

fn smtp_error(error: smtp::Error) -> MailError {
    if matches!(error.status().map(u16::from), Some(530 | 534 | 535 | 538)) {
        MailError::Authentication
    } else if error.is_tls() || error.is_client() {
        MailError::Configuration
    } else if error.is_transient() {
        MailError::Transient
    } else if error.is_permanent() {
        MailError::Rejected
    } else {
        // A lost connection can occur after DATA was accepted. Do not promise safe retries.
        MailError::Uncertain
    }
}

#[cfg(test)]
mod tests;
