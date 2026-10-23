use rapidmaker_notifications::{EmailPayload, MailError, Mailer};
use rapidmaker_storage::{EmailOutcome, Storage};
use std::{sync::Arc, time::Duration};
use tokio::{sync::watch, task::JoinHandle};

pub struct MailWorker {
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}
impl MailWorker {
    pub fn start(db: Storage, mailer: Arc<Mailer>) -> Self {
        let (stop, mut receiver) = watch::channel(false);
        let task = tokio::spawn(async move {
            loop {
                if *receiver.borrow() {
                    break;
                }
                if process(&db, &mailer).await {
                    break;
                }
                tokio::select! {
                    _=tokio::time::sleep(Duration::from_secs(6))=>{},
                    _=receiver.changed()=>{break;}
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
            Ok(Err(_)) => eprintln!("event=email_worker_stopped reason=task_failure"),
            Err(_) => {
                task.abort();
                eprintln!("event=email_worker_stopped reason=shutdown_timeout");
            }
        }
    }
}
// Return true only when operator intervention is needed; never log provider text or addresses.
async fn process(db: &Storage, mailer: &Mailer) -> bool {
    let now = super::auth::now();
    if db.cleanup_email(now).await.is_err() {
        eprintln!("event=email_cleanup_failed");
    }
    let mail = match db.claim_email(now).await {
        Ok(Some(v)) => v,
        Ok(None) => return false,
        Err(_) => {
            eprintln!("event=email_claim_failed");
            return false;
        }
    };
    let allowed = match db.email_allowed(&mail).await {
        Ok(v) => v,
        Err(_) => {
            eprintln!("event=email_preferences_failed");
            return false;
        }
    };
    let (mut paused, mut outcome) = (false, EmailOutcome::Skipped);
    if allowed {
        outcome = match serde_json::from_value::<EmailPayload>(mail.payload.clone()) {
            Err(_) => EmailOutcome::Failed("payload"),
            Ok(payload) => match mailer
                .send_queued(&mail.recipient, &payload, &mail.id)
                .await
            {
                Ok(()) => EmailOutcome::Sent,
                Err(MailError::Disabled | MailError::Configuration | MailError::Authentication) => {
                    paused = true;
                    EmailOutcome::Deferred("configuration")
                }
                Err(MailError::Transient | MailError::Uncertain | MailError::Busy) => {
                    EmailOutcome::Retry("temporary")
                }
                Err(_) => EmailOutcome::Failed("rejected"),
            },
        };
    }
    if db
        .finish_email(&mail, outcome, super::auth::now())
        .await
        .is_err()
    {
        eprintln!("event=email_finish_failed");
    }
    if paused {
        eprintln!("event=email_worker_paused reason=configuration");
    }
    paused
}
