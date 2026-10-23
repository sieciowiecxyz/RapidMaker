//! Vetted WebAuthn passkeys require user verification in registration and authentication.
//! Serialized challenge state stays exclusively in PostgreSQL, never in browser responses.
pub use webauthn_rs::prelude::*;
