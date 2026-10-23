use utoipa::openapi::{
    OpenApi, Server,
    security::{ApiKey, ApiKeyValue, SecurityScheme},
};
use utoipa_swagger_ui::{Config, SwaggerUi};

pub(super) struct BinaryFile;

impl utoipa::PartialSchema for BinaryFile {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        use utoipa::openapi::schema::{ObjectBuilder, SchemaFormat, SchemaType, Type};
        ObjectBuilder::new()
            .schema_type(SchemaType::Type(Type::String))
            .format(Some(SchemaFormat::Custom("binary".into())))
            .into()
    }
}
impl utoipa::ToSchema for BinaryFile {}

pub fn router(mut document: OpenApi) -> axum::Router {
    document.info.title = "Rapidmaker API".into();
    document.info.version = env!("CARGO_PKG_VERSION").into();
    document.info.contact = None;
    document.info.license = None;
    document.info.description = Some("Logowanie lokalne: otwórz http://localhost:18430/api zgodnie z AUTH_ORIGIN. Cookie HttpOnly ustawia backend po logowaniu. Dla zapisu sesyjnego podaj X-CSRF-Token z /api/auth/me przez Authorize. Administracja dodatkowo wymaga passkey. Wywołania Try it out wykonują rzeczywiste operacje API.".into());
    document.servers = Some(vec![Server::new("/")]);
    let components = document.components.get_or_insert_with(Default::default);
    components.add_security_scheme(
        "session",
        SecurityScheme::ApiKey(ApiKey::Cookie(
            ApiKeyValue::new(super::auth::session_name()),
        )),
    );
    components.add_security_scheme(
        "guest",
        SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new(
            if std::env::var("AUTH_MODE").as_deref() == Ok("production") {
                "__Host-rapidmaker_guest"
            } else {
                "rapidmaker_guest"
            },
        ))),
    );
    components.add_security_scheme(
        "csrf",
        SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-CSRF-Token"))),
    );
    axum::Router::new().merge(
        SwaggerUi::new("/api")
            .url("/api/openapi.json", document)
            .config(
                Config::default()
                    .filter(true)
                    .validator_url("")
                    .with_credentials(true)
                    .persist_authorization(false),
            ),
    )
}
