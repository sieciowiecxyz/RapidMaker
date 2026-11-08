//! Guest-owned private models. Cookies authorize reads; filenames never choose disk paths.
use super::model_limits::ModelProfile;
use axum::{
    Json,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Extension, FromRequest, Path as RoutePath, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use rapidmaker_storage::{ModelCommitUncertain, ModelRemoval, Storage, StoredModel};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, DirBuilder, File, OpenOptions, ReadDir},
    io::{SeekFrom, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt},
    sync::{Semaphore, SemaphorePermit},
};
use utoipa_axum::{router::OpenApiRouter as Router, routes};
use uuid::Uuid;

const MAX_PREVIEW_BYTES: usize = 25 * 1024 * 1024;
const COOKIE: &str = "rapidmaker_guest";
static UPLOAD_BUSY: Semaphore = Semaphore::const_new(1);
static DOWNLOADS: Semaphore = Semaphore::const_new(4);
const DOWNLOAD_CHUNK_BYTES: usize = 64 * 1024;
type Error = (StatusCode, Json<Value>);

#[derive(Clone)]
struct FileState {
    storage: Option<Storage>,
    root: PathBuf,
    profile: ModelProfile,
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
    let state = FileState {
        storage,
        root,
        profile: ModelProfile::from_env()?,
    };
    let uploads = Router::new()
        .routes(routes!(upload))
        .route_layer(middleware::from_fn_with_state(state.clone(), admit_upload));
    Ok(Router::new()
        .merge(uploads)
        .routes(routes!(remove))
        .routes(routes!(metadata))
        .routes(routes!(source))
        .routes(routes!(preview))
        .layer(DefaultBodyLimit::max(state.profile.input_bytes()))
        .with_state(state))
}

// Acquired before Bytes extraction, rather than after allocating parallel large bodies.
async fn admit_upload(State(state): State<FileState>, request: Request, next: Next) -> Response {
    if let Err(error) = database(&state) {
        return error.into_response();
    }
    if let Err(error) = filename(request.headers()) {
        return error.into_response();
    }
    let Ok(_permit) = UPLOAD_BUSY.try_acquire() else {
        return upload_busy();
    };
    let (parts, body) = request.into_parts();
    // Bound ingestion only: cancelling a handler during a database commit can
    // leave a committed model whose PendingDirectory has already been removed.
    let bytes = match upload_body(Request::from_parts(parts.clone(), body)).await {
        Ok(bytes) => bytes,
        Err(response) => return response,
    };
    // Bytes extraction and Body::from(Bytes) share the allocation.
    next.run(Request::from_parts(parts, Body::from(bytes)))
        .await
}

async fn upload_body(request: Request) -> Result<Bytes, Response> {
    match tokio::time::timeout(Duration::from_secs(60), Bytes::from_request(request, &())).await {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(rejection)) => Err(error(
            if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
                StatusCode::PAYLOAD_TOO_LARGE
            } else {
                StatusCode::BAD_REQUEST
            },
            "Nie można odczytać przesyłanego modelu.",
        )
        .into_response()),
        Err(_) => Err(error(
            StatusCode::REQUEST_TIMEOUT,
            "Przesyłanie modelu trwa zbyt długo. Spróbuj ponownie.",
        )
        .into_response()),
    }
}

// Only refusals known to precede model creation are safe for automatic resends.
fn upload_busy() -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, "1")],
        Json(json!({
            "code": "model_upload_busy",
            "message": "Trwa przesyłanie modelu. Spróbuj za chwilę."
        })),
    )
        .into_response()
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

#[derive(Default)]
pub(super) struct ModelCleanupCursor {
    scan: Option<(PathBuf, u64, u64, ReadDir)>,
}

// A fresh open is essential: duplicated FDs share an advisory filesystem lock.
fn lock_model_root(root: &Path) -> std::io::Result<Option<File>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(root)?;
    if file.metadata()?.permissions().mode() & 0o077 != 0 {
        return Err(std::io::Error::other("model root is not private"));
    }
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}

pub(super) async fn cleanup_models(
    storage: &Storage,
    cursor: &mut ModelCleanupCursor,
) -> Result<(), Error> {
    let state = FileState {
        storage: Some(storage.clone()),
        profile: ModelProfile::from_env()
            .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE, "Invalid model profile"))?,
        root: model_root().map_err(|_| {
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Magazyn modeli jest niedostępny.",
            )
        })?,
    };
    let timestamp = now()?;
    cleanup(&state, timestamp).await?;
    cleanup_orphans(&state, timestamp, cursor).await
}

async fn cleanup_orphans(
    state: &FileState,
    timestamp: i64,
    cursor: &mut ModelCleanupCursor,
) -> Result<(), Error> {
    let unavailable = || {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Nie można posprzątać magazynu modeli.",
        )
    };
    let Some(root) = lock_model_root(&state.root).map_err(|_| unavailable())? else {
        return Ok(());
    };
    let identity = root.metadata().map_err(|_| unavailable())?;
    if !cursor.scan.as_ref().is_some_and(|(path, dev, ino, _)| {
        path == &state.root && *dev == identity.dev() && *ino == identity.ino()
    }) {
        cursor.scan = Some((
            state.root.clone(),
            identity.dev(),
            identity.ino(),
            fs::read_dir(&state.root).map_err(|_| unavailable())?,
        ));
    }
    let scan = &mut cursor.scan.as_mut().unwrap().3;
    let mut candidates = Vec::new();
    let mut finished = false;
    // Count every entry, including junk and protected directories, for fair bounded work.
    for _ in 0..64 {
        let Some(entry) = scan.next() else {
            finished = true;
            break;
        };
        let entry = entry.map_err(|_| unavailable())?;
        let Some(id) = entry
            .file_name()
            .to_str()
            .filter(|id| valid_id(id))
            .map(str::to_owned)
        else {
            continue;
        };
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(unavailable()),
        };
        if !metadata.is_dir() {
            continue;
        }
        let modified = metadata.modified().map_err(|_| unavailable())?;
        let Some(cutoff) = u64::try_from(timestamp)
            .ok()
            .and_then(|now| now.checked_sub(86400))
            .and_then(|age| UNIX_EPOCH.checked_add(Duration::from_secs(age)))
        else {
            continue;
        };
        if modified <= cutoff {
            candidates.push(id);
        }
    }
    if finished {
        cursor.scan = None;
    }
    if candidates.is_empty() {
        return Ok(());
    }
    // Only this read-only future is cancellable, never a model writer/COMMIT.
    let eligible = tokio::time::timeout(
        Duration::from_secs(5),
        database(state)?.orphan_model_ids(&candidates, timestamp),
    )
    .await
    .map_err(|_| unavailable())?
    .map_err(|_| unavailable())?;
    let mut removed = false;
    for id in eligible {
        match fs::remove_dir_all(state.root.join(&id)) {
            Ok(()) => {
                removed = true;
                tracing::info!(event = "model_orphan_removed", model_id = %id);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                tracing::error!(event = "model_orphan_cleanup_failed", model_id = %id, reason = "filesystem")
            }
        }
    }
    if removed {
        root.sync_all().map_err(|_| unavailable())?;
    }
    Ok(())
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
        remove_private_directory(&state.root, &id)?;
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

fn remove_private_directory(root: &Path, id: &str) -> Result<(), Error> {
    if !valid_id(id) {
        return Err(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Błędny identyfikator modelu.",
        ));
    }
    match fs::remove_dir_all(root.join(id)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Nie można usunąć plików modelu. Spróbuj ponownie.",
        )),
    }
}

#[utoipa::path(operation_id = "files_remove", delete, path = "/api/models/{id}", tag = "Modele",
    params(("id" = String, Path)),
    responses((status = 204, description = "Model i pliki usunięte"),
        (status = 404, description = "Brak własnego modelu", body = Value),
        (status = 409, description = "Model chroniony przez wycenę lub zamówienie", body = Value),
        (status = 403, description = "Nieprawidłowy Origin lub CSRF", body = Value),
        (status = 500, description = "Usuwanie plików nie powiodło się; ponów żądanie", body = Value),
        (status = 503, description = "Magazyn chwilowo niedostępny", body = Value)),
    security(("session" = [], "csrf" = []), ("guest" = [])))]
async fn remove(
    State(state): State<FileState>,
    Extension(auth): Extension<std::sync::Arc<super::auth::Auth>>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<String>,
) -> Result<StatusCode, Error> {
    if !valid_id(&id) {
        return Err(error(StatusCode::NOT_FOUND, "Nie znaleziono modelu."));
    }
    // Destructive guest requests also require an exact Origin in localhost mode.
    auth.check_origin(&headers)?;
    let guest_owner = if super::auth::session_cookie(&headers)?.is_some() {
        auth.mutation(&headers).await?;
        owner(&headers)
    } else {
        None
    };
    let storage = database(&state)?;
    let owner = super::cart::owner(Some(storage), &headers).await?;
    let timestamp = now()?;
    let unavailable = || {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Nie można usunąć modelu. Spróbuj ponownie.",
        )
    };
    match storage
        .begin_model_removal_with_guest(&id, &owner, guest_owner.as_deref(), timestamp)
        .await
        .map_err(|_| unavailable())?
    {
        ModelRemoval::Missing => {
            return Err(error(StatusCode::NOT_FOUND, "Nie znaleziono modelu."));
        }
        ModelRemoval::Protected => {
            return Err(error(
                StatusCode::CONFLICT,
                "Model jest używany przez wycenę lub zamówienie.",
            ));
        }
        ModelRemoval::Ready => {}
    }
    remove_private_directory(&state.root, &id)?;
    storage
        .forget_expired_model(&id, timestamp)
        .await
        .map_err(|_| unavailable())?;
    Ok(StatusCode::NO_CONTENT)
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
    request_body(content = super::docs::BinaryFile, content_type = "application/octet-stream", description = "Oryginał domyślnie do 32 MiB (lokalny profil large: 96 MiB)"),
    responses((status = 201, description = "Prywatny model i adres GLB", body = Value),
        (status = 408, description = "Przekroczony czas odbierania pliku", body = Value),
        (status = 413, description = "Przekroczony limit pliku", body = Value),
        (status = 429, description = "Trwa inny upload", body = Value),
        (status = 503, description = "Magazyn chwilowo niedostępny", body = Value),
        (status = 422, description = "Nieprawidłowa geometria", body = Value)))]
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
    // Declared before PendingDirectory so rollback removes files before unlocking.
    let Some(_root_lock) = lock_model_root(&state.root).map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Nie można zapisać modelu.",
        )
    })?
    else {
        return Ok(upload_busy());
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
    let prepared = super::models::prepare_for_slicing(&source, &mesh, state.profile)
        .await
        .map_err(|message| {
            if message == "Model parser busy" {
                error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "Trwa analiza innego modelu. Spróbuj za chwilę.",
                )
            } else if message == "Model topology invalid" {
                (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({
                    "code": "model_topology_invalid",
                    "message": "Siatka modelu wymaga naprawy. Napraw model i prześlij plik ponownie."
                })))
            } else if message == "Model resource limit" {
                (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({
                    "code": "model_resource_limit",
                    "message": "Model przekracza limit złożoności lub rozpakowanego pliku. Zmniejsz szczegółowość siatki lub wyeksportuj tylko potrzebne części i spróbuj ponownie."
                })))
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
    // Persist contents, then the child directory, then its entry in the storage root.
    for path in [
        &mesh,
        &directory.join("preview.glb"),
        &directory,
        &state.root,
    ] {
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
    if !storage
        .insert_model_with_limits(
            &record,
            state.profile.input_bytes(),
            state.profile.stl_bytes(),
            || pending.0 = None,
        )
        .await
        .map_err(|failure| {
            if failure.is::<ModelCommitUncertain>() {
                // Files were retained before COMMIT; the worker reconciles aged orphans.
                tracing::error!(event="model_commit_uncertain", model_id=%record.id, reason="prepared files retained for reconciliation");
            }
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Nie można zachować modelu.",
            )
        })?
    {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "Przekroczono limit zapisanych modeli. Spróbuj po wygaśnięciu starszych plików.",
        ));
    }
    tracing::info!(event="model_uploaded", model_id=%record.id, source_bytes=record.source_bytes, mesh_bytes=record.mesh_bytes, preview_bytes=record.preview_bytes);
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
        (status = 404, description = "Brak własnego modelu", body = Value), (status = 410, description = "Plik jest niedostępny", body = Value),
        (status = 503, description = "Limit równoczesnych pobrań", body = Value)),
    security(("session" = []), ("guest" = [])))]
async fn source(
    State(state): State<FileState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<String>,
) -> Result<Response, Error> {
    let handle = resolve(database(&state)?, &headers, &id).await?;
    source_response(&handle).await
}

// Persisted artifacts remain readable after switching back to standard admission.
async fn source_response(handle: &ModelHandle) -> Result<Response, Error> {
    let file = private_file(
        &handle.source,
        handle.record.source_bytes,
        ModelProfile::Large.input_bytes(),
    )
    .await?;
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
        file.body(),
    )
        .into_response())
}

#[utoipa::path(operation_id = "files_preview", get, path = "/api/models/{id}/preview.glb", tag = "Modele", params(("id" = String, Path)),
    responses((status = 200, description = "GLB w metrach/Y-up", body = super::docs::BinaryFile, content_type = "model/gltf-binary"),
        (status = 404, description = "Brak własnego modelu", body = Value), (status = 409, description = "Starszy upload wymaga ponownego przesłania", body = Value),
        (status = 503, description = "Limit równoczesnych pobrań", body = Value)),
    security(("session" = []), ("guest" = [])))]
async fn preview(
    State(state): State<FileState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<String>,
) -> Result<Response, Error> {
    let handle = resolve(database(&state)?, &headers, &id).await?;
    preview_response(&handle).await
}

async fn preview_response(handle: &ModelHandle) -> Result<Response, Error> {
    if handle.record.preview_bytes == 0 {
        return Err(error(
            StatusCode::CONFLICT,
            "Ten model nie ma jeszcze podglądu. Prześlij plik ponownie.",
        ));
    }
    let mut file = private_file(
        &handle.mesh.with_file_name("preview.glb"),
        handle.record.preview_bytes,
        MAX_PREVIEW_BYTES,
    )
    .await?;
    if validate_glb(&mut file.file, file.remaining).await.is_err() {
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
        file.body(),
    )
        .into_response())
}

pub(super) async fn order_artifact(
    record: StoredModel,
    kind: &str,
    admin: bool,
) -> Result<Response, Error> {
    let handle = handle(record)?;
    artifact_response(&handle, kind, admin).await
}

async fn artifact_response(
    handle: &ModelHandle,
    kind: &str,
    admin: bool,
) -> Result<Response, Error> {
    match kind {
        "source" => source_response(handle).await,
        "preview.glb" => preview_response(handle).await,
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
            private_file(
                &handle.mesh,
                handle.record.mesh_bytes,
                ModelProfile::Large.stl_bytes(),
            )
            .await?
            .body(),
        )
            .into_response()),
        _ => Err(error(
            StatusCode::NOT_FOUND,
            "Nie znaleziono pliku zamówienia.",
        )),
    }
}

// An independent producer bounds the lifetime of the file and download permit.
struct PrivateFile {
    file: tokio::fs::File,
    remaining: u64,
    _permit: SemaphorePermit<'static>,
}

impl PrivateFile {
    fn body(self) -> ([(header::HeaderName, String); 1], Body) {
        self.body_at(tokio::time::Instant::now() + Duration::from_secs(300))
    }

    fn body_at(
        mut self,
        deadline: tokio::time::Instant,
    ) -> ([(header::HeaderName, String); 1], Body) {
        let length = self.remaining;
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let transfer = async {
                while self.remaining > 0 {
                    let mut bytes =
                        vec![0; self.remaining.min(DOWNLOAD_CHUNK_BYTES as u64) as usize];
                    let read = self.file.read(&mut bytes).await?;
                    if read == 0 {
                        return Err(download_error());
                    }
                    bytes.truncate(read);
                    sender
                        .send(Bytes::from(bytes))
                        .await
                        .map_err(|_| download_error())?;
                    self.remaining -= read as u64;
                }
                Ok::<_, std::io::Error>(())
            };
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => {},
                _ = sender.closed() => {},
                _ = transfer => {},
            }
            // Release resources before channel closure tells the receiver that transfer ended.
            drop(self);
            drop(sender);
        });
        let stream = futures_util::stream::try_unfold(
            (receiver, length),
            |(mut receiver, remaining)| async move {
                if remaining == 0 {
                    // EOF follows producer cleanup, even if the final chunk was consumed first.
                    if receiver.recv().await.is_some() {
                        return Err(download_error());
                    }
                    return Ok(None);
                }
                let bytes = receiver.recv().await.ok_or_else(download_error)?;
                let remaining = remaining
                    .checked_sub(bytes.len() as u64)
                    .ok_or_else(download_error)?;
                Ok(Some((bytes, (receiver, remaining))))
            },
        );
        (
            [(header::CONTENT_LENGTH, length.to_string())],
            Body::from_stream(stream),
        )
    }
}

fn download_error() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "Private model read failed",
    )
}

async fn private_file(path: &Path, expected: i64, limit: usize) -> Result<PrivateFile, Error> {
    let invalid = || {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Nieprawidłowy zapis modelu.",
        )
    };
    if expected <= 0 || expected as u64 > limit as u64 {
        return Err(invalid());
    }
    let permit = DOWNLOADS.try_acquire().map_err(|_| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Trwa pobieranie modeli. Spróbuj za chwilę.",
        )
    })?;
    // Do not follow a replaced final symlink; nonblocking open also rejects FIFOs
    // through the same-descriptor regular-file check without blocking a worker.
    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                error(
                    StatusCode::GONE,
                    "Plik modelu jest niedostępny. Prześlij go ponownie.",
                )
            } else {
                invalid()
            }
        })?;
    let metadata = file.metadata().await.map_err(|_| invalid())?;
    if !metadata.is_file() || metadata.len() != expected as u64 {
        return Err(invalid());
    }
    Ok(PrivateFile {
        file,
        remaining: expected as u64,
        _permit: permit,
    })
}

// Equivalent envelope checks to gltf::binary::Glb::from_slice, without copying
// JSON/BIN payloads. That parser permits trailing bytes and does not parse JSON.
async fn validate_glb(file: &mut tokio::fs::File, length: u64) -> std::io::Result<()> {
    let mut header = [0; 12];
    file.read_exact(&mut header).await?;
    let declared = u32::from_le_bytes(header[8..12].try_into().unwrap()) as u64;
    if &header[..4] != b"glTF"
        || header[4..8] != 2u32.to_le_bytes()
        || declared < 12
        || declared > length
    {
        return Err(download_error());
    }
    let mut chunk = [0; 8];
    file.read_exact(&mut chunk).await?;
    let json_end = 20 + u32::from_le_bytes(chunk[..4].try_into().unwrap()) as u64;
    if &chunk[4..] != b"JSON" || json_end > length {
        return Err(download_error());
    }
    if json_end < length {
        file.seek(SeekFrom::Start(json_end)).await?;
        file.read_exact(&mut chunk).await?;
        let bin_end = json_end + 8 + u32::from_le_bytes(chunk[..4].try_into().unwrap()) as u64;
        if &chunk[4..] != b"BIN\0" || bin_end > length {
            return Err(download_error());
        }
    }
    file.seek(SeekFrom::Start(0)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    static DOWNLOAD_TEST: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[test]
    fn private_directory_removal_is_uuid_bounded_idempotent_and_retryable() {
        let root = std::env::temp_dir().join(format!("rapidmaker-remove-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let _cleanup = PendingDirectory(Some(root.clone()));
        let id = Uuid::new_v4().to_string();
        let directory = root.join(&id);
        fs::write(&directory, b"blocked by non-directory").unwrap();
        assert!(remove_private_directory(&root, &id).is_err());
        assert!(remove_private_directory(&root, "..").is_err());
        assert!(directory.exists());
        fs::remove_file(&directory).unwrap();
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("model.stl"), b"private model").unwrap();
        remove_private_directory(&root, &id).unwrap();
        assert!(!directory.exists());
        remove_private_directory(&root, &id).unwrap();
        let keep = root.join("keep");
        fs::create_dir(&keep).unwrap();
        fs::write(keep.join("sentinel"), b"preserve").unwrap();
        std::os::unix::fs::symlink(&keep, &directory).unwrap();
        remove_private_directory(&root, &id).unwrap();
        assert!(keep.join("sentinel").is_file());
        assert!(root.is_dir());
    }

    #[tokio::test]
    async fn persisted_large_downloads_ignore_admission_profile_and_keep_admin_gate() {
        let _serial = DOWNLOAD_TEST.lock().await;
        let directory = std::env::temp_dir().join(format!("rapidmaker-read-{}", Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let _cleanup = PendingDirectory(Some(directory.clone()));
        let source_len = ModelProfile::Standard.input_bytes() + 1;
        let mesh_len = ModelProfile::Standard.stl_bytes() + 1;
        let mut handle = ModelHandle {
            source: directory.join("model.stl"),
            mesh: directory.join("canonical.stl"),
            record: StoredModel {
                id: Uuid::new_v4().to_string(),
                owner_hash: "a".repeat(64),
                user_id: None,
                filename: "large.stl".into(),
                extension: "stl".into(),
                source_bytes: source_len as i64,
                mesh_bytes: mesh_len as i64,
                preview_bytes: 0,
                sha256: "a".repeat(64),
                dimensions_mm: [20.; 3],
                expires_at: 2_000_000_000,
            },
        };
        for (path, length) in [(&handle.source, source_len), (&handle.mesh, mesh_len)] {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .unwrap()
                .set_len(length as u64)
                .unwrap();
        }
        for (kind, admin, expected) in [("source", false, source_len), ("mesh", true, mesh_len)] {
            let response = artifact_response(&handle, kind, admin).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "private, no-store"
            );
            assert_eq!(
                response.headers()[header::CONTENT_LENGTH],
                expected.to_string()
            );
            let mut body = response.into_body().into_data_stream();
            let mut total = 0;
            while let Some(bytes) = body.next().await {
                let bytes = bytes.unwrap();
                assert!(bytes.len() <= DOWNLOAD_CHUNK_BYTES);
                assert!(bytes.iter().all(|byte| *byte == 0));
                total += bytes.len();
            }
            assert_eq!(total, expected);
        }
        assert_eq!(
            artifact_response(&handle, "mesh", false)
                .await
                .unwrap_err()
                .0,
            StatusCode::NOT_FOUND
        );
        handle.record.source_bytes += 1;
        assert_eq!(
            source_response(&handle).await.unwrap_err().0,
            StatusCode::INTERNAL_SERVER_ERROR
        );
        handle.record.source_bytes = ModelProfile::Large.input_bytes() as i64 + 1;
        assert_eq!(
            source_response(&handle).await.unwrap_err().0,
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    include!("files_transfer_tests.rs");

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

#[cfg(test)]
#[path = "files_orphan_tests.rs"]
mod orphan_tests;
