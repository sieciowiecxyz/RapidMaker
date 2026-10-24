//! One bounded IMAP batch; PostgreSQL owns the cursor, lease and delivery deduplication.
use rapidmaker_notifications::{Imap, ImapCursor, InboundDisposition};
use rapidmaker_storage::{InboxOutcome, Storage};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::{sync::watch, task::JoinHandle};

type Failure = Box<dyn std::error::Error + Send + Sync>;
#[derive(Default, Serialize)]
pub struct MailReceived {
    imported: usize,
    duplicate: usize,
    rejected: usize,
    review: usize,
    ignored: usize,
}
pub struct InboundWorker {
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}
impl InboundWorker {
    pub fn start(db: Storage, imap: Imap) -> Self {
        let (stop, mut receiver) = watch::channel(false);
        let task = tokio::spawn(async move {
            loop {
                if *receiver.borrow() {
                    break;
                }
                tokio::select! {
                    result = receive(&db, &imap) => {
                        if let Ok(counts) = &result
                            && counts.imported + counts.duplicate + counts.rejected + counts.review > 0 {
                            eprintln!("event=mail_inbound_batch imported={} duplicate={} rejected={} review={}", counts.imported, counts.duplicate, counts.rejected, counts.review);
                        }
                        if let Err(error) = result {
                            if matches!(error.downcast_ref::<rapidmaker_notifications::MailError>(), Some(rapidmaker_notifications::MailError::Authentication | rapidmaker_notifications::MailError::Configuration | rapidmaker_notifications::MailError::TestRecipient)) {
                                eprintln!("event=mail_inbound_paused reason=configuration");
                                break;
                            }
                            eprintln!("event=mail_inbound_failed reason=receive");
                        }
                    }
                    _ = receiver.changed() => break,
                }
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(30)) => {},
                    _ = receiver.changed() => break,
                }
            }
        });
        Self { stop, task }
    }
    pub async fn shutdown(self) {
        let _ = self.stop.send(true);
        let mut task = self.task;
        match tokio::time::timeout(Duration::from_secs(25), &mut task).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => eprintln!("event=mail_inbound_stopped reason=task_failure"),
            Err(_) => {
                task.abort();
                eprintln!("event=mail_inbound_stopped reason=shutdown_timeout");
            }
        }
    }
}

/// Local diagnostic, using the same lease and import path as the embedded worker.
pub async fn receive_once() -> Result<serde_json::Value, Failure> {
    let imap = Imap::from_env()?.ok_or("mail inbound disabled")?;
    let db = Storage::connect(&std::env::var("DATABASE_URL")?).await?;
    Ok(serde_json::to_value(receive(&db, &imap).await?)?)
}
async fn receive(db: &Storage, imap: &Imap) -> Result<MailReceived, Failure> {
    let baseline = imap.poll(None).await?.cursor;
    let Some(lease) = db
        .inbox_claim(
            &imap.mailbox_id(),
            i64::from(baseline.uid_validity),
            i64::from(baseline.next_uid.saturating_sub(1)),
            super::auth::now(),
        )
        .await?
    else {
        return Ok(MailReceived::default());
    };
    let result = async {
        let next_uid = u32::try_from(lease.last_uid)?
            .checked_add(1)
            .ok_or("invalid IMAP cursor")?;
        let batch = imap
            .poll(Some(ImapCursor {
                uid_validity: baseline.uid_validity,
                next_uid,
            }))
            .await?;
        if batch.cursor.uid_validity != baseline.uid_validity {
            return Ok(MailReceived::default());
        }
        let mut received = MailReceived::default();
        let operator = std::env::var("MAIL_REPLY_TO").ok();
        for mail in batch.items {
            let digest = Sha256::digest(serde_json::to_vec(&(
                &mail.from,
                &mail.message_id,
                &mail.references,
                &mail.body,
            ))?);
            let reply = mail.disposition == InboundDisposition::Reply;
            let route = reply.then(|| mail.references.first()).flatten();
            let review = (mail.disposition == InboundDisposition::Review).then_some(mail.reason);
            match db
                .inbox_receive(
                    &lease,
                    i64::from(mail.uid),
                    &digest,
                    route.map(String::as_str),
                    mail.from.as_deref().unwrap_or(""),
                    reply.then_some(mail.body.as_deref()).flatten(),
                    review,
                    operator.as_deref(),
                    super::auth::now(),
                )
                .await?
            {
                InboxOutcome::Imported => received.imported += 1,
                InboxOutcome::Duplicate => received.duplicate += 1,
                InboxOutcome::Rejected => received.rejected += 1,
                InboxOutcome::Review => received.review += 1,
                InboxOutcome::Ignored => received.ignored += 1,
            }
        }
        if !db
            .inbox_advance(
                &lease,
                i64::from(batch.cursor.next_uid.saturating_sub(1)),
                super::auth::now(),
            )
            .await?
        {
            return Err("IMAP lease lost".into());
        }
        Ok(received)
    }
    .await;
    let released = db.inbox_release(&lease).await;
    match result {
        Ok(value) => {
            released?;
            Ok(value)
        }
        Err(e) => Err(e),
    }
}
