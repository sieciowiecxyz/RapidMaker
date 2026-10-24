use super::*;

fn configured(overrides: &[(&str, &str)]) -> Result<Mailer, MailError> {
    let defaults = [
        ("MAIL_MODE", "test"),
        ("MAIL_SMTP_HOST", "smtp.mail.me.com"),
        ("MAIL_SMTP_PORT", "587"),
        ("MAIL_USER", "smtp-login@example.com"),
        ("MAIL_PASSWORD", "not-a-real-password"),
        ("MAIL_FROM", "sender@example.com"),
        ("MAIL_REPLY_TO", "support@example.com"),
        ("MAIL_TEST_TO", "void@sieciowiec.xyz"),
    ];
    Mailer::from_values(|key| {
        Ok(overrides
            .iter()
            .chain(defaults.iter())
            .find(|(k, _)| *k == key)
            .map(|(_, v)| (*v).into()))
    })
}

#[test]
fn rejects_bad_configuration_and_addresses() {
    for pair in [
        ("MAIL_MODE", "typo"),
        ("MAIL_SMTP_PORT", "25"),
        ("MAIL_SMTP_HOST", "localhost"),
        ("MAIL_PASSWORD", ""),
        ("MAIL_TEST_TO", ""),
        ("MAIL_FROM", "a@example.com\r\nBcc: b@example.com"),
    ] {
        assert!(configured(&[pair]).is_err());
    }
    for value in [
        "",
        "not-an-email",
        "a@example.com,b@example.com",
        "Name <a@example.com>",
        "a@example.com\n",
        &format!("{}@example.com", "a".repeat(255)),
    ] {
        assert!(address(value).is_err(), "accepted an invalid recipient");
    }
}

#[test]
fn test_mode_rejects_other_recipient_and_builds_private_plain_text() {
    let mailer = configured(&[]).unwrap();
    let sender = mailer.enabled.as_ref().unwrap();
    assert!(matches!(
        sender.message("other@example.com", Notification::LoginCode("12345678")),
        Err(MailError::TestRecipient)
    ));
    let message = sender
        .message("void@sieciowiec.xyz", Notification::LoginCode("12345678"))
        .unwrap();
    let raw = String::from_utf8(message.formatted()).unwrap();
    assert!(raw.contains("From: sender@example.com"));
    assert!(raw.contains("Reply-To: support@example.com"));
    assert!(raw.contains("To: void@sieciowiec.xyz"));
    assert!(raw.contains("text/plain") && raw.contains("12345678"));
    assert!(!raw.contains("smtp-login") && !raw.contains("not-a-real-password"));
}

#[tokio::test]
async fn disabled_mail_never_claims_success_and_test_command_requires_test_mode() {
    let disabled = Mailer::from_values(|_| Ok(None)).unwrap();
    assert_eq!(
        disabled
            .send("void@sieciowiec.xyz", Notification::Test)
            .await,
        Err(MailError::Disabled)
    );
    assert_eq!(
        configured(&[("MAIL_MODE", "live")])
            .unwrap()
            .send_test()
            .await,
        Err(MailError::TestRecipient)
    );
}

#[test]
fn templates_validate_codes_and_keep_payment_separate_from_order_confirmation() {
    for code in ["123", "1234567x", "1234567\n"] {
        assert!(Notification::LoginCode(code).render().is_err());
    }
    assert!(
        Notification::StatusChanged {
            reference: "RM-1",
            status: "ready\r\n"
        }
        .render()
        .is_err()
    );
    let (_, body) = Notification::PaymentReceived {
        reference: "RM-1",
        amount_grosze: 1001,
    }
    .render()
    .unwrap();
    assert!(body.contains("10.01 PLN") && body.contains("RM-1"));
    let (_, body) = Notification::OrderConfirmed { reference: "RM-1" }
        .render()
        .unwrap();
    assert!(body.contains("nie oznacza potwierdzenia płatności"));
}

#[test]
fn outbox_payload_rejects_unknown_version_and_preserves_message_id() {
    let payload = EmailPayload {
        version: 1,
        notification: EmailNotification::OrderConfirmed {
            reference: "RM-1".into(),
        },
    };
    assert!(payload.validate().is_ok());
    assert!(
        EmailPayload {
            version: 2,
            notification: EmailNotification::OrderConfirmed {
                reference: "RM-1".into()
            }
        }
        .validate()
        .is_err()
    );
    let sender = configured(&[]).unwrap();
    let message = sender
        .enabled
        .as_ref()
        .unwrap()
        .message_with_id(
            "void@sieciowiec.xyz",
            payload.notification.borrowed(),
            Some("<stable-id@rapidmaker.pl>".into()),
        )
        .unwrap();
    let text = String::from_utf8(message.formatted()).unwrap();
    assert!(text.contains("Message-ID: <stable-id@rapidmaker.pl>"));
}

#[test]
fn chat_copy_has_order_link_content_and_a_reply_reference_that_survives_mime() {
    let route = "11111111-2222-4333-8444-555555555555";
    let key = "01234567890123456789012345678901";
    let reference = reply_reference(route, key).unwrap();
    let mailer = configured(&[("MAIL_REPLY_KEY", key)]).unwrap();
    for (operator, path) in [
        (true, "/admin/zamowienia/RM-2"),
        (false, "/konto/zamowienia/RM-2"),
    ] {
        let notification = Notification::ChatMessage {
            reference: "RM-2",
            body: "Hello from chat",
            operator,
        };
        let (subject, body) = notification.render().unwrap();
        assert!(
            subject.contains("RM-2") && body.contains(path) && body.contains("Hello from chat")
        );
        let message = mailer
            .enabled
            .as_ref()
            .unwrap()
            .message_with_id(
                "void@sieciowiec.xyz",
                Notification::ChatMessage {
                    reference: "RM-2",
                    body: "Hello from chat",
                    operator,
                },
                Some(reference.clone()),
            )
            .unwrap();
        let bytes = message.formatted();
        let parsed = mail_parser::MessageParser::default().parse(&bytes).unwrap();
        assert_eq!(
            verify_reply_reference(parsed.message_id().unwrap(), key).as_deref(),
            Some(route)
        );
    }
}

#[test]
fn dispatched_email_contains_all_tracking_numbers_and_rejects_header_injection() {
    let numbers = vec!["DPD123".to_string(), "DPD456".to_string()];
    let notification = EmailPayload {
        version: 1,
        notification: EmailNotification::ShipmentDispatched {
            reference: "RM-1".into(),
            carrier: "DPD".into(),
            tracking_numbers: numbers,
            remaining_packages: 1,
        },
    };
    notification.validate().unwrap();
    let (subject, body) = notification.notification.borrowed().render().unwrap();
    assert!(subject.contains("przewoźnikowi"));
    assert!(body.contains("DPD123") && body.contains("DPD456"));
    let bad = vec!["number\r\nBcc: other@example.invalid".to_string()];
    assert!(
        Notification::ShipmentDispatched {
            reference: "RM-1",
            carrier: "DPD",
            tracking_numbers: &bad,
            remaining_packages: 0
        }
        .render()
        .is_err()
    );
}
