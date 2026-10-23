//! Guest-owned private models. Cookies authorize reads; filenames never choose disk paths.
use axum::{
    Json,
    body::Bytes,
    extract::{DefaultBodyLimit, Path as RoutePath, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use rapidmaker_storage::{Storage, StoredModel};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, DirBuilder, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Semaphore;
use utoipa_axum::{router::OpenApiRouter as Router, routes};
use uuid::Uuid;

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_PREVIEW_BYTES: usize = 25 * 1024 * 1024;
const COOKIE: &str = "rapidmaker_guest";
static UPLOAD_BUSY: Semaphore = Semaphore::const_new(1);
type Error = (StatusCode, Json<Value>);

#[derive(Clone)]
struct FileState {
    storage: Option<Storage>,
    root: PathBuf,
}

pub struct ModelHandle {
    pub record: StoredModel,
    pub source: PathBuf,
    pub mesh: PathBuf,
}

pub fn router(storage: Option<Storage>) -> Result<Router, String> {
    let root = model_root()?;
    if storage.is_some() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&root)
            .map_err(|e| format!("Cannot prepare private model storage: {e}"))?;
        let metadata = fs::symlink_metadata(&root).map_err(|e| e.to_string())?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err("Model storage must be a private directory (0700)".into());
        }
    }
    let state = FileState { storage, root };
    let uploads = Router::new()
        .routes(routes!(upload))
        .route_layer(middleware::from_fn_with_state(state.clone(), admit_upload));
    Ok(Router::new()
        .merge(uploads)
        .routes(routes!(metadata))
        .routes(routes!(source))
        .routes(routes!(preview))
        .layer(DefaultBodyLimit::max(MAX_BYTES))
        .with_state(state))
}

// Acquired before Bytes extraction, rather than after allocating parallel large bodies.
async fn admit_upload(State(state): State<FileState>, request: Request, next: Next) -> Response {
    if let Err(error) = database(&state) {
        return error.into_response();
    }
    let Ok(_permit) = UPLOAD_BUSY.try_acquire() else {
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            "Trwa przesyłanie modelu. Spróbuj za chwilę.",
        )
        .into_response();
    };
    next.run(request).await
}

fn error(status: StatusCode, message: &str) -> Error {
    (status, Json(json!({"message": message})))
}

fn database(state: &FileState) -> Result<&Storage, Error> {
    state.storage.as_ref().ok_or_else(|| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Magazyn modeli jest niedostępny.",
        )
    })
}

fn model_root() -> Result<PathBuf, String> {
    let root = if let Some(path) = std::env::var_os("MODEL_STORAGE_DIR") {
        PathBuf::from(path)
    } else {
        PathBuf::from(std::env::var_os("HOME").ok_or("Missing HOME or MODEL_STORAGE_DIR")?)
            .join(".local/state/rapidmaker/models")
    };
    if !root.is_absolute() {
        return Err("MODEL_STORAGE_DIR must be absolute".into());
    }
    Ok(root)
}

fn now() -> Result<i64, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
        .ok_or_else(|| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Nie można ustalić czasu ważności modelu.",
            )
        })
}

/// Only a complete opaque token is accepted; malformed/ambiguous cookies authorize nothing.
fn guest_cookie_name() -> &'static str {
    if std::env::var("AUTH_MODE").as_deref() == Ok("production") {
        "__Host-rapidmaker_guest"
    } else {
        COOKIE
    }
}
fn guest_token(headers: &HeaderMap) -> Option<&str> {
    let mut token = None;
    for cookie in headers.get_all(header::COOKIE) {
        for pair in cookie.to_str().ok()?.split(';') {
            let Some((name, value)) = pair.trim().split_once('=') else {
                continue;
            };
            if name != guest_cookie_name() {
                continue;
            }
            if token.is_some() || value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return None;
            }
            token = Some(value);
        }
    }
    token
}

pub fn owner(headers: &HeaderMap) -> Option<String> {
    guest_token(headers).map(|value| format!("{:x}", Sha256::digest(value.as_bytes())))
}

struct PendingDirectory(Option<PathBuf>);
impl Drop for PendingDirectory {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = fs::remove_dir_all(path);
        }
    }
}

pub(super) async fn cleanup_models(storage: &Storage) -> Result<(), Error> {
    let state = FileState {
        storage: Some(storage.clone()),
        root: model_root().map_err(|_| {
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Magazyn modeli jest niedostępny.",
            )
        })?,
    };
    cleanup(&state, now()?).await
}

async fn cleanup(state: &FileState, timestamp: i64) -> Result<(), Error> {
    let storage = database(state)?;
    let ids = storage.expired_model_ids(timestamp).await.map_err(|_| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Nie można posprzątać magazynu modeli.",
        )
    })?;
    for id in ids {
        // Defense in depth: a corrupt DB identifier must never become a disk path.
        if !valid_id(&id) {
            return Err(error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Błędny identyfikator modelu.",
            ));
        }
        match fs::remove_dir_all(state.root.join(&id)) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => {
                return Err(error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Nie można usunąć wygasłego modelu.",
                ));
            }
        }
        storage
            .forget_expired_model(&id, timestamp)
            .await
            .map_err(|_| {
                error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Nie można posprzątać magazynu modeli.",
                )
            })?;
    }
    Ok(())
}

fn valid_id(id: &str) -> bool {
    Uuid::parse_str(id).is_ok_and(|uuid| uuid.to_string() == id)
}

fn filename(headers: &HeaderMap) -> Result<(String, String), Error> {
    let encoded = headers
        .get("x-model-filename")
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.len() <= 1024)
        .ok_or_else(|| error(StatusCode::BAD_REQUEST, "Podaj nazwę pliku modelu."))?;
    let name = percent_decode_str(encoded)
        .decode_utf8()
        .map_err(|_| error(StatusCode::BAD_REQUEST, "Nieprawidłowa nazwa pliku."))?
        .into_owned();
    let extension = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if name.is_empty()
        || name.len() > 255
        || name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\'))
        || !matches!(extension.as_str(), "stl" | "step" | "stp" | "3mf")
    {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Wybierz plik STL, STEP lub 3MF z poprawną nazwą.",
        ));
    }
    Ok((name, extension))
}

#[utoipa::path(operation_id = "files_upload", post, path = "/api/models", tag = "Modele",
    params(("x-model-filename" = String, Header, description = "Zakodowana procentowo nazwa STL, STEP lub 3MF")),
    request_body(content = super::docs::BinaryFile, content_type = "application/octet-stream", description = "Oryginał do 32 MiB"),
    responses((status = 201, description = "Prywatny model i adres GLB", body = Value),
        (status = 413, description = "Przekroczony limit pliku"), (status = 422, description = "Nieprawidłowa geometria", body = Value)))]
async fn upload(
    State(state): State<FileState>,
    headers: HeaderMap,
    input: Bytes,
) -> Result<Response, Error> {
    let storage = database(&state)?;
    let (filename, extension) = filename(&headers)?;
    if input.is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "Plik modelu jest pusty."));
    }
    let timestamp = now()?;
    cleanup(&state, timestamp).await?;
    let token = guest_token(&headers)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()));
    let (owner_hash, user_id) = if super::auth::session_cookie(&headers)?.is_some() {
        super::cart::identity(Some(storage), &headers).await?
    } else {
        (format!("{:x}", Sha256::digest(token.as_bytes())), None)
    };
    let id = Uuid::new_v4().to_string();
    let directory = state.root.join(&id);
    DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Nie można zapisać modelu.",
            )
        })?;
    let mut pending = PendingDirectory(Some(directory.clone()));
    let source = directory.join(format!("model.{extension}"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&source)
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Nie można zapisać modelu.",
            )
        })?;
    file.write_all(&input)
        .and_then(|_| file.sync_all())
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Nie można zapisać modelu.",
            )
        })?;
    let mesh = directory.join("canonical.stl");
    let prepared = super::models::prepare_for_slicing(&source, &mesh)
        .await
        .map_err(|message| {
            if message == "Model parser busy" {
                error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "Trwa analiza innego modelu. Spróbuj za chwilę.",
                )
            } else if message == "Model conversion unavailable" {
                error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Proces analizy modeli jest niedostępny.",
                )
            } else {
                error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Nie można odczytać geometrii. Wybierz poprawny STL, STEP lub 3MF.",
                )
            }
        })?;
    let mesh_bytes = fs::metadata(&mesh)
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Brak siatki modelu."))?
        .len();
    let preview_bytes = fs::metadata(directory.join("preview.glb"))
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Brak podglądu modelu."))?
        .len();
    for path in [&mesh, &directory.join("preview.glb"), &directory] {
        fs::File::open(path)
            .and_then(|file| file.sync_all())
            .map_err(|_| {
                error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Nie można utrwalić modelu.",
                )
            })?;
    }
    let record = StoredModel {
        id,
        owner_hash,
        user_id,
        filename,
        extension,
        source_bytes: input.len() as i64,
        mesh_bytes: mesh_bytes as i64,
        preview_bytes: preview_bytes as i64,
        sha256: format!("{:x}", Sha256::digest(&input)),
        dimensions_mm: prepared.dimensions_mm,
        expires_at: timestamp + 86400,
    };
    if !storage.insert_model(&record).await.map_err(|_| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Nie można zachować modelu.",
        )
    })? {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "Przekroczono limit zapisanych modeli. Spróbuj po wygaśnięciu starszych plików.",
        ));
    }
    pending.0 = None;
    let mut response = (StatusCode::CREATED, Json(public_metadata(&record))).into_response();
    if record.user_id.is_none() {
        response.headers_mut().insert(
            header::SET_COOKIE,
            HeaderValue::from_str(&format!(
                "{}={token}; HttpOnly; SameSite=Strict; Path={}; Max-Age=86400{}",
                guest_cookie_name(),
                if std::env::var("AUTH_MODE").as_deref() == Ok("production") {
                    "/"
                } else {
                    "/api"
                },
                if std::env::var("AUTH_MODE").as_deref() == Ok("production") {
                    "; Secure"
                } else {
                    ""
                }
            ))
            .map_err(|_| {
                error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Nie można utworzyć sesji modelu.",
                )
            })?,
        );
    }
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}

pub async fn resolve(
    storage: &Storage,
    headers: &HeaderMap,
    id: &str,
) -> Result<ModelHandle, Error> {
    let owner = super::cart::owner(Some(storage), headers).await?;
    if !valid_id(id) {
        return Err(error(StatusCode::NOT_FOUND, "Nie znaleziono modelu."));
    }
    let record = storage
        .model(id, &owner, now()?)
        .await
        .map_err(|_| {
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Magazyn modeli jest niedostępny.",
            )
        })?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "Nie znaleziono modelu lub wygasł."))?;
    handle(record)
}

pub(super) fn handle(record: StoredModel) -> Result<ModelHandle, Error> {
    if !valid_id(&record.id) || !matches!(record.extension.as_str(), "stl" | "step" | "stp" | "3mf")
    {
        return Err(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Błędne metadane modelu.",
        ));
    }
    let directory = model_root()
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Magazyn modeli jest niedostępny.",
            )
        })?
        .join(&record.id);
    Ok(ModelHandle {
        source: directory.join(format!("model.{}", record.extension)),
        mesh: directory.join("canonical.stl"),
        record,
    })
}

fn public_metadata(record: &StoredModel) -> Value {
    json!({"id":record.id,"filename":record.filename,"dimensions_mm":record.dimensions_mm,
        "weight_g":null,"geometry_source":"parsed_model","weight_source":"slicer_pending",
        "reason":"Masa będzie wyznaczona przez slicer.","expires_at":record.expires_at,
        "preview_bytes":record.preview_bytes,
        "preview_url":(record.preview_bytes > 0).then(|| format!("/api/models/{}/preview.glb",record.id))})
}

#[utoipa::path(operation_id = "files_metadata", get, path = "/api/models/{id}", tag = "Modele", params(("id" = String, Path)),
    responses((status = 200, description = "Wymiary, ważność i preview_url", body = Value), (status = 404, description = "Model nie istnieje, wygasł lub jest cudzy", body = Value)),
    security(("session" = []), ("guest" = [])))]
async fn metadata(
    State(state): State<FileState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<String>,
) -> Result<Response, Error> {
    let handle = resolve(database(&state)?, &headers, &id).await?;
    Ok((
        [(header::CACHE_CONTROL, "private, no-store")],
        Json(public_metadata(&handle.record)),
    )
        .into_response())
}

#[utoipa::path(operation_id = "files_source", get, path = "/api/models/{id}/source", tag = "Modele", params(("id" = String, Path)),
    responses((status = 200, description = "Oryginał", body = super::docs::BinaryFile, content_type = "application/octet-stream"),
        (status = 404, description = "Brak własnego modelu", body = Value), (status = 410, description = "Plik jest niedostępny", body = Value)),
    security(("session" = []), ("guest" = [])))]
async fn source(
    State(state): State<FileState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<String>,
) -> Result<Response, Error> {
    let handle = resolve(database(&state)?, &headers, &id).await?;
    source_response(&handle)
}

fn source_response(handle: &ModelHandle) -> Result<Response, Error> {
    let bytes = private_bytes(&handle.source, handle.record.source_bytes, MAX_BYTES)?;
    let disposition = HeaderValue::from_str(&format!(
        "attachment; filename=\"model.{}\"; filename*=UTF-8''{}",
        handle.record.extension,
        utf8_percent_encode(&handle.record.filename, NON_ALPHANUMERIC)
    ))
    .map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Nieprawidłowa nazwa modelu.",
        )
    })?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            ),
            (header::CONTENT_DISPOSITION, disposition),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, no-store"),
            ),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        bytes,
    )
        .into_response())
}

#[utoipa::path(operation_id = "files_preview", get, path = "/api/models/{id}/preview.glb", tag = "Modele", params(("id" = String, Path)),
    responses((status = 200, description = "GLB w metrach/Y-up", body = super::docs::BinaryFile, content_type = "model/gltf-binary"),
        (status = 404, description = "Brak własnego modelu", body = Value), (status = 409, description = "Starszy upload wymaga ponownego przesłania", body = Value)),
    security(("session" = []), ("guest" = [])))]
async fn preview(
    State(state): State<FileState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<String>,
) -> Result<Response, Error> {
    let handle = resolve(database(&state)?, &headers, &id).await?;
    preview_response(&handle)
}

fn preview_response(handle: &ModelHandle) -> Result<Response, Error> {
    if handle.record.preview_bytes == 0 {
        return Err(error(
            StatusCode::CONFLICT,
            "Ten model nie ma jeszcze podglądu. Prześlij plik ponownie.",
        ));
    }
    let bytes = private_bytes(
        &handle.mesh.with_file_name("preview.glb"),
        handle.record.preview_bytes,
        MAX_PREVIEW_BYTES,
    )?;
    if gltf::binary::Glb::from_slice(&bytes).is_err() {
        return Err(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Nieprawidłowy zapis podglądu.",
        ));
    }
    Ok((
        [
            (header::CONTENT_TYPE, "model/gltf-binary"),
            (header::CONTENT_DISPOSITION, "inline; filename=preview.glb"),
            (header::CACHE_CONTROL, "private, no-store"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        bytes,
    )
        .into_response())
}

pub(super) fn order_artifact(
    record: StoredModel,
    kind: &str,
    admin: bool,
) -> Result<Response, Error> {
    let handle = handle(record)?;
    match kind {
        "source" => source_response(&handle),
        "preview.glb" => preview_response(&handle),
        "mesh" if admin => Ok((
            [
                (header::CONTENT_TYPE, "application/octet-stream"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=production.stl",
                ),
                (header::CACHE_CONTROL, "private, no-store"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            private_bytes(&handle.mesh, handle.record.mesh_bytes, 50 * 1024 * 1024)?,
        )
            .into_response()),
        _ => Err(error(
            StatusCode::NOT_FOUND,
            "Nie znaleziono pliku zamówienia.",
        )),
    }
}

fn private_bytes(path: &Path, expected: i64, limit: usize) -> Result<Vec<u8>, Error> {
    let invalid = || {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Nieprawidłowy zapis modelu.",
        )
    };
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        error(
            StatusCode::GONE,
            "Plik modelu jest niedostępny. Prześlij go ponownie.",
        )
    })?;
    if expected <= 0
        || expected as u64 > limit as u64
        || !metadata.is_file()
        || metadata.len() != expected as u64
    {
        return Err(invalid());
    }
    let file = fs::File::open(path).map_err(|_| invalid())?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    if bytes.len() as i64 != expected {
        return Err(invalid());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_tokens_are_opaque_and_ambiguous_cookies_authorize_nothing() {
        let mut headers = HeaderMap::new();
        assert!(owner(&headers).is_none());
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{COOKIE}={}", "a".repeat(64))).unwrap(),
        );
        assert_eq!(owner(&headers).unwrap().len(), 64);
        headers.append(
            header::COOKIE,
            HeaderValue::from_str(&format!("{COOKIE}={}", "b".repeat(64))).unwrap(),
        );
        assert!(owner(&headers).is_none());
    }

    #[test]
    fn names_are_decoded_but_never_allow_paths_or_response_header_controls() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-model-filename",
            HeaderValue::from_static("cz%C4%99%C5%9B%C4%87.STL"),
        );
        assert_eq!(
            filename(&headers).unwrap(),
            ("część.STL".into(), "stl".into())
        );
        for name in ["..%2Fmodel.stl", "bad%0A.stl", "bad%5Cmodel.stl", "bad.svg"] {
            headers.insert("x-model-filename", HeaderValue::from_str(name).unwrap());
            assert!(filename(&headers).is_err());
        }
        assert!(!valid_id("../model"));
    }
}
