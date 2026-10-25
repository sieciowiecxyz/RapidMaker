//! Immutable documents: keep old versions available for delayed outbox delivery.
use crate::MailError;

pub const VERSION: &str = "2026-10-25";
pub const PRODUCTION_DAYS: u32 = 14;
pub const TERMS: &str = include_str!("../../../data/legal/2026-10-25/regulamin.txt");
pub const PRIVACY: &str = include_str!("../../../data/legal/2026-10-25/prywatnosc.txt");

pub fn documents(version: &str) -> Result<String, MailError> {
    if version != VERSION {
        return Err(MailError::InvalidMessage);
    }
    Ok(format!("{TERMS}\n\n{PRIVACY}"))
}
