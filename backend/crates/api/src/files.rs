//! Guest-owned private models. Cookies authorize reads; filenames never choose disk paths.
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path as RoutePath, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use rapidmaker_storage::{Storage, StoredModel};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, DirBuilder, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Semaphore;
use uuid::Uuid;

const MAX_BYTES: usize = 32 * 1024 * 1024;
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
    Ok(Router::new()
        .route(
            "/api/models",
            post(upload).layer(middleware::from_fn_with_state(state.clone(), admit_upload)),
        )
        .route("/api/models/{id}", get(metadata))
        .route("/api/models/{id}/source", get(source))
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
fn guest_token(headers: &HeaderMap) -> Option<&str> {
    let mut token = None;
    for cookie in headers.get_all(header::COOKIE) {
        for pair in cookie.to_str().ok()?.split(';') {
            let Some((name, value)) = pair.trim().split_once('=') else {
                continue;
            };
            if name != COOKIE {
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
    let owner_hash = format!("{:x}", Sha256::digest(token.as_bytes()));
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
    let record = StoredModel {
        id,
        owner_hash,
        filename,
        extension,
        source_bytes: input.len() as i64,
        mesh_bytes: mesh_bytes as i64,
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
    {
        response.headers_mut().insert(
            header::SET_COOKIE,
            HeaderValue::from_str(&format!(
                "{COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/api; Max-Age=86400"
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
    let owner = owner(headers).ok_or_else(|| {
        error(
            StatusCode::UNAUTHORIZED,
            "Brak sesji modelu. Prześlij model ponownie.",
        )
    })?;
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
        "reason":"Masa będzie wyznaczona przez slicer.","expires_at":record.expires_at})
}

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

async fn source(
    State(state): State<FileState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<String>,
) -> Result<Response, Error> {
    let handle = resolve(database(&state)?, &headers, &id).await?;
    let file = fs::File::open(&handle.source).map_err(|_| {
        error(
            StatusCode::GONE,
            "Plik modelu jest niedostępny. Prześlij go ponownie.",
        )
    })?;
    if file
        .metadata()
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Nie można odczytać modelu.",
            )
        })?
        .len()
        != handle.record.source_bytes as u64
    {
        return Err(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Nieprawidłowy zapis modelu.",
        ));
    }
    // One bounded source read; never serve an HTML/SVG document inline.
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Nie można odczytać modelu.",
            )
        })?;
    if bytes.len() as i64 != handle.record.source_bytes {
        return Err(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Nieprawidłowy zapis modelu.",
        ));
    }
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
