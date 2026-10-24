use crate::{InboundDisposition, InboundMail, MailError, inbound::MAX_MAIL_BYTES};
use hmac::{Hmac, Mac};
use mail_parser::{HeaderName, MessageParser, MimeHeaders};
use sha2::Sha256;

/// Recipient-specific UUID route, authenticated independently of From/Message-ID.
pub fn reply_reference(route: &str, key: &str) -> Result<String, MailError> {
    if !valid_route(route) || key.len() < 32 {
        return Err(MailError::Configuration);
    }
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key.as_bytes()).map_err(|_| MailError::Configuration)?;
    mac.update(b"rapidmaker-chat-v1:");
    mac.update(route.as_bytes());
    let signature: String = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(format!("<rm1.{route}.{signature}@rapidmaker.pl>"))
}
pub fn verify_reply_reference(reference: &str, key: &str) -> Option<String> {
    if key.len() < 32 || reference.len() > 160 {
        return None;
    }
    let text = reference
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>');
    let text = text.strip_prefix("rm1.")?.strip_suffix("@rapidmaker.pl")?;
    let (route, signature) = text.split_once('.')?;
    if !valid_route(route) || signature.len() != 64 {
        return None;
    }
    let signature: Option<Vec<_>> = signature
        .as_bytes()
        .chunks_exact(2)
        .map(|v| {
            let h = (v[0] as char).to_digit(16)?;
            let l = (v[1] as char).to_digit(16)?;
            Some((h * 16 + l) as u8)
        })
        .collect();
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).ok()?;
    mac.update(b"rapidmaker-chat-v1:");
    mac.update(route.as_bytes());
    mac.verify_slice(&signature?).ok()?;
    Some(route.to_owned())
}
fn valid_route(route: &str) -> bool {
    route.len() == 36
        && route.bytes().enumerate().all(|(i, v)| {
            if [8, 13, 18, 23].contains(&i) {
                v == b'-'
            } else {
                v.is_ascii_hexdigit()
            }
        })
}
/// MIME parsing never grants access: storage must verify the current route participant.
pub fn parse_inbound(uid: u32, raw: &[u8], key: &str, test_mode: bool) -> InboundMail {
    let mut result = InboundMail::rejected(uid, "malformed");
    if raw.len() > MAX_MAIL_BYTES {
        result.reason = "oversize";
        return result;
    }
    let Some(message) = MessageParser::default().parse(raw) else {
        return result;
    };
    result.message_id = message
        .message_id()
        .filter(|v| v.len() <= 254 && !v.chars().any(char::is_control))
        .map(str::to_owned);
    if message
        .headers()
        .iter()
        .filter(|v| v.name == HeaderName::From)
        .count()
        != 1
    {
        result.reason = "ambiguous_sender";
        return result;
    }
    let addresses: Vec<_> = message
        .from()
        .into_iter()
        .flat_map(|v| v.iter())
        .filter_map(|v| v.address())
        .collect();
    if addresses.len() == 1 && crate::valid_address(addresses[0]) {
        result.from = Some(addresses[0].to_ascii_lowercase());
    }
    let ignore = |mut value: InboundMail, reason| {
        value.disposition = InboundDisposition::Ignored;
        value.reason = reason;
        value
    };
    if message
        .header("Auto-Submitted")
        .and_then(|v| v.as_text())
        .is_some_and(|v| !v.eq_ignore_ascii_case("no"))
        || message
            .header("Precedence")
            .and_then(|v| v.as_text())
            .is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "bulk" | "list" | "junk"))
        || message.header("X-Autoreply").is_some()
        || message.header("X-Autorespond").is_some()
        || message.header(HeaderName::ListId).is_some()
        || message.content_type().is_some_and(|v| {
            v.c_type.eq_ignore_ascii_case("multipart")
                && v.c_subtype
                    .as_deref()
                    .is_some_and(|s| s.eq_ignore_ascii_case("report"))
        })
    {
        return ignore(result, "automatic");
    }
    // Outgoing copies are never accepted as replies, even when sent to our own test mailbox.
    if result
        .message_id
        .as_deref()
        .and_then(|v| verify_reply_reference(v, key))
        .is_some()
    {
        return ignore(result, "own_copy");
    }
    for value in [message.in_reply_to(), message.references()] {
        for reference in value.as_text_list().into_iter().flatten() {
            if let Some(route) = verify_reply_reference(reference, key)
                && !result.references.contains(&route)
            {
                result.references.push(route);
            }
        }
        // In-Reply-To identifies the immediate notification; References may include older routes.
        if !result.references.is_empty() {
            break;
        }
    }
    if result.references.is_empty() {
        return ignore(result, "unrelated");
    }
    if result.references.len() != 1 {
        result.reason = "ambiguous_route";
        return result;
    }
    if result.from.is_none() {
        result.reason = "ambiguous_sender";
        return result;
    }
    if test_mode && result.from.as_deref() != Some("void@sieciowiec.xyz") {
        result.reason = "test_sender";
        return result;
    }
    if message.attachment_count() > 0 {
        result.reason = "attachment";
        return result;
    }
    if message.parts.len() > 32 {
        result.reason = "mime_parts";
        return result;
    }
    if mime_depth(&message, 0) > 8 {
        result.reason = "mime_depth";
        return result;
    }
    if message.text_body.len().max(message.html_body.len()) != 1 {
        result.reason = "ambiguous_body";
        return result;
    }
    let Some(body) = message.body_text(0) else {
        result.reason = "empty";
        return result;
    };
    let body = clean_reply(&body);
    if body.is_empty()
        || body.len() > 4000
        || body
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        result.reason = "body_size";
        return result;
    }
    result.body = Some(body);
    result.disposition = InboundDisposition::Reply;
    result.reason = "reply";
    result
}
fn mime_depth(message: &mail_parser::Message<'_>, part: usize) -> usize {
    match &message.parts[part].body {
        mail_parser::PartType::Multipart(children) => {
            1 + children
                .iter()
                .map(|v| mime_depth(message, *v as usize))
                .max()
                .unwrap_or(0)
        }
        mail_parser::PartType::Message(_) => 9,
        _ => 1,
    }
}
fn clean_reply(body: &str) -> String {
    let normalized = body.replace("\r\n", "\n");
    let lines: Vec<_> = normalized.lines().collect();
    let end = lines
        .iter()
        .position(|line| {
            line.trim_start().starts_with('>')
                || *line == "-- "
                || *line == "-----Original Message-----"
                || line.starts_with("On ") && line.ends_with("wrote:")
                || line.starts_with("W dniu ")
                    && (line.ends_with("napisał:") || line.ends_with("napisała:"))
        })
        .unwrap_or(lines.len());
    lines[..end].join("\n").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    const KEY: &str = "12345678901234567890123456789012";
    const ROUTE: &str = "12345678-1234-1234-1234-123456789abc";
    #[test]
    fn references_are_authenticated_and_replies_are_cleaned() {
        let reference = reply_reference(ROUTE, KEY).unwrap();
        assert_eq!(
            verify_reply_reference(&reference, KEY).as_deref(),
            Some(ROUTE)
        );
        assert!(verify_reply_reference(&reference.replace("rm1.1", "rm1.2"), KEY).is_none());
        let raw = format!(
            "From: void@sieciowiec.xyz\r\nMessage-ID: <reply@icloud.com>\r\nIn-Reply-To: {reference}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nTak, drukujemy.\r\n\r\nOn Monday wrote:\r\n> Stary tekst"
        );
        let reply = parse_inbound(3, raw.as_bytes(), KEY, true);
        assert_eq!(reply.disposition, InboundDisposition::Reply);
        assert_eq!(reply.body.as_deref(), Some("Tak, drukujemy."));
        assert_eq!(reply.references, vec![ROUTE]);
        let auto = raw.replace("From:", "Auto-Submitted: auto-replied\r\nFrom:");
        assert_eq!(
            parse_inbound(3, auto.as_bytes(), KEY, true).disposition,
            InboundDisposition::Ignored
        );
        let own = raw.replace("<reply@icloud.com>", &reference);
        assert_eq!(
            parse_inbound(3, own.as_bytes(), KEY, true).reason,
            "own_copy"
        );
    }
    #[test]
    fn attachments_and_ambiguous_or_oversize_messages_are_not_imported() {
        let reference = reply_reference(ROUTE, KEY).unwrap();
        let raw = format!(
            "From: void@sieciowiec.xyz, other@example.com\r\nIn-Reply-To: {reference}\r\n\r\nTak"
        );
        assert_eq!(
            parse_inbound(1, raw.as_bytes(), KEY, true).reason,
            "ambiguous_sender"
        );
        assert_eq!(
            parse_inbound(1, &vec![b'x'; MAX_MAIL_BYTES + 1], KEY, true).reason,
            "oversize"
        );
        let raw = format!(
            "From: void@sieciowiec.xyz\r\nIn-Reply-To: {reference}\r\nContent-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nTak\r\n--x\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=a\r\n\r\nAA\r\n--x--\r\n"
        );
        assert_eq!(
            parse_inbound(1, raw.as_bytes(), KEY, true).reason,
            "attachment"
        );
    }
}
