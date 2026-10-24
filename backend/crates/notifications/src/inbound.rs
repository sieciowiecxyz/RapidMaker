//! Read-only iCloud inbox polling. Cursor persistence and authorization belong to storage.
use crate::{MailError, parse_inbound};
use futures_util::TryStreamExt;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
    sync::Semaphore,
};
use tokio_rustls::{
    TlsConnector,
    rustls::{ClientConfig, RootCertStore, pki_types::ServerName},
};

pub const MAX_MAIL_BYTES: usize = 256 * 1024;
const HOST: &str = "imap.mail.me.com";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImapCursor {
    pub uid_validity: u32,
    pub next_uid: u32,
}
pub struct ImapBatch {
    pub cursor: ImapCursor,
    pub items: Vec<InboundMail>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InboundDisposition {
    Reply,
    Ignored,
    Review,
}
/// Deliberately no Debug: message content and private routing references are sensitive.
pub struct InboundMail {
    pub uid: u32,
    pub message_id: Option<String>,
    pub from: Option<String>,
    pub references: Vec<String>,
    pub body: Option<String>,
    pub disposition: InboundDisposition,
    pub reason: &'static str,
}
impl InboundMail {
    pub(crate) fn rejected(uid: u32, reason: &'static str) -> Self {
        Self {
            uid,
            message_id: None,
            from: None,
            references: Vec::new(),
            body: None,
            disposition: InboundDisposition::Review,
            reason,
        }
    }
}
pub struct Imap {
    user: String,
    password: String,
    reply_key: String,
    test_mode: bool,
    mailbox: String,
    slot: Semaphore,
}
impl Imap {
    pub fn from_env() -> Result<Option<Self>, MailError> {
        let mode = std::env::var("MAIL_INBOUND_MODE").unwrap_or_else(|_| "disabled".into());
        if mode == "disabled" {
            return Ok(None);
        }
        if !matches!(mode.as_str(), "test" | "live") {
            return Err(MailError::Configuration);
        }
        let required = |name| {
            std::env::var(name)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .ok_or(MailError::Configuration)
        };
        if std::env::var("MAIL_IMAP_HOST").is_ok_and(|v| v != HOST)
            || std::env::var("MAIL_IMAP_PORT").is_ok_and(|v| v != "993")
        {
            return Err(MailError::Configuration);
        }
        let mailbox = std::env::var("MAIL_IMAP_MAILBOX").unwrap_or_else(|_| "INBOX".into());
        if mailbox.is_empty() || mailbox.len() > 128 || mailbox.chars().any(char::is_control) {
            return Err(MailError::Configuration);
        }
        let reply_key = required("MAIL_REPLY_KEY")?;
        if reply_key.len() < 32 {
            return Err(MailError::Configuration);
        }
        if mode == "test" && !required("MAIL_TEST_TO")?.eq_ignore_ascii_case("void@sieciowiec.xyz")
        {
            return Err(MailError::TestRecipient);
        }
        Ok(Some(Self {
            user: std::env::var("MAIL_IMAP_USER")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .map(Ok)
                .unwrap_or_else(|| required("MAIL_USER"))?,
            password: std::env::var("MAIL_IMAP_PASSWORD")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .map(Ok)
                .unwrap_or_else(|| required("MAIL_PASSWORD"))?,
            reply_key,
            test_mode: mode == "test",
            mailbox,
            slot: Semaphore::new(1),
        }))
    }
    pub fn mailbox_id(&self) -> String {
        use sha2::{Digest, Sha256};
        Sha256::digest(format!(
            "{HOST}:993:{}:{}",
            self.user.to_ascii_lowercase(),
            self.mailbox
        ))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
    }
    /// Initial or changed UIDVALIDITY establishes a baseline, never imports old mail.
    pub async fn poll(&self, cursor: Option<ImapCursor>) -> Result<ImapBatch, MailError> {
        let _slot = self.slot.try_acquire().map_err(|_| MailError::Busy)?;
        tokio::time::timeout(Duration::from_secs(30), self.poll_inner(cursor))
            .await
            .map_err(|_| MailError::Transient)?
    }
    async fn poll_inner(&self, cursor: Option<ImapCursor>) -> Result<ImapBatch, MailError> {
        let roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let config = ClientConfig::builder_with_provider(Arc::new(
            tokio_rustls::rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| MailError::Configuration)?
        .with_root_certificates(roots)
        .with_no_client_auth();
        let tcp = TcpStream::connect((HOST, 993))
            .await
            .map_err(|_| MailError::Transient)?;
        let tls = TlsConnector::from(Arc::new(config))
            .connect(
                ServerName::try_from(HOST).map_err(|_| MailError::Configuration)?,
                tcp,
            )
            .await
            .map_err(|_| MailError::Transient)?;
        // A hostile/corrupt response cannot make the protocol decoder buffer without bound.
        let mut client = async_imap::Client::new(Bounded {
            stream: tls,
            remaining: 8 * 1024 * 1024,
        });
        client
            .read_response()
            .await
            .map_err(|_| MailError::Transient)?
            .ok_or(MailError::Transient)?;
        let mut session = client
            .login(&self.user, &self.password)
            .await
            .map_err(|_| MailError::Authentication)?;
        let mailbox = session
            .examine(&self.mailbox)
            .await
            .map_err(|_| MailError::Transient)?;
        let current = ImapCursor {
            uid_validity: mailbox.uid_validity.ok_or(MailError::Transient)?,
            next_uid: mailbox.uid_next.ok_or(MailError::Transient)?,
        };
        let Some(cursor) = cursor.filter(|c| c.uid_validity == current.uid_validity) else {
            session.logout().await.map_err(|_| MailError::Transient)?;
            return Ok(ImapBatch {
                cursor: current,
                items: vec![],
            });
        };
        if cursor.next_uid == 0 || cursor.next_uid > current.next_uid {
            return Err(MailError::Configuration);
        }
        let end = cursor
            .next_uid
            .saturating_add(19)
            .min(current.next_uid.saturating_sub(1));
        let mut items = Vec::new();
        if cursor.next_uid < current.next_uid {
            let range = format!("{}:{end}", cursor.next_uid);
            let mut metadata: Vec<_> = session
                .uid_fetch(range, "(UID RFC822.SIZE)")
                .await
                .map_err(|_| MailError::Transient)?
                .try_collect()
                .await
                .map_err(|_| MailError::Transient)?;
            metadata.sort_by_key(|item| item.uid);
            for item in metadata {
                let uid = item.uid.ok_or(MailError::Transient)?;
                if uid < cursor.next_uid || uid > end {
                    return Err(MailError::Transient);
                }
                if item.size.is_none_or(|v| v as usize > MAX_MAIL_BYTES) {
                    items.push(InboundMail::rejected(uid, "oversize"));
                    continue;
                }
                let bodies: Vec<_> = session
                    .uid_fetch(uid.to_string(), "(UID BODY.PEEK[]<0.262145>)")
                    .await
                    .map_err(|_| MailError::Transient)?
                    .try_collect()
                    .await
                    .map_err(|_| MailError::Transient)?;
                if let Some(body) = bodies
                    .iter()
                    .find(|v| v.uid == Some(uid))
                    .and_then(|v| v.body())
                {
                    items.push(parse_inbound(uid, body, &self.reply_key, self.test_mode));
                } else {
                    return Err(MailError::Transient);
                }
            }
        }
        session.logout().await.map_err(|_| MailError::Transient)?;
        Ok(ImapBatch {
            cursor: ImapCursor {
                uid_validity: current.uid_validity,
                next_uid: if cursor.next_uid < current.next_uid {
                    end + 1
                } else {
                    current.next_uid
                },
            },
            items,
        })
    }
}
#[derive(Debug)]
struct Bounded<T> {
    stream: T,
    remaining: usize,
}
impl<T: AsyncRead + Unpin> AsyncRead for Bounded<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        if this.remaining == 0 {
            return Poll::Ready(Err(std::io::Error::other("IMAP response limit")));
        }
        let allowed = this.remaining.min(buf.remaining());
        let mut part = buf.take(allowed);
        match Pin::new(&mut this.stream).poll_read(cx, &mut part) {
            Poll::Ready(Ok(())) => {
                let n = part.filled().len();
                this.remaining -= n;
                buf.advance(n);
                Poll::Ready(Ok(()))
            }
            result => result,
        }
    }
}
impl<T: AsyncWrite + Unpin> AsyncWrite for Bounded<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().stream).poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
    }
}
