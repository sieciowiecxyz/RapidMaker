//! Bounded geometry analysis in a disposable subprocess. The shipping mass is a
//! conservative solid estimate, never a slicer result or customer-provided value.
use axum::{
    Json,
    body::Bytes,
    extract::DefaultBodyLimit,
    http::{HeaderMap, StatusCode},
};
use rapidmaker_geometry::{
    ConversionLimits, ThreeMfLimits, source_to_preview_glb, source_to_production,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::sync::Semaphore;
use utoipa_axum::{router::OpenApiRouter as Router, routes};

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_PREVIEW_BYTES: usize = 25 * 1024 * 1024;
static BUSY: Semaphore = Semaphore::const_new(1);

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ModelAnalysis {
    pub id: String,
    pub dimensions_mm: [f64; 3],
    pub weight_g: Option<f64>,
    pub geometry_source: String,
    pub weight_source: String,
    pub reason: Option<String>,
}

pub fn router() -> Router {
    Router::new()
        .routes(routes!(analyze))
        .layer(DefaultBodyLimit::max(MAX_BYTES))
}

type Error = (StatusCode, Json<Value>);
fn error(status: StatusCode, message: &str) -> Error {
    (status, Json(json!({"message": message})))
}
struct Workspace(PathBuf);
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[utoipa::path(operation_id = "models_analyze", post, path = "/api/models/analyze", tag = "Modele",
    params(("x-model-filename" = String, Header, description = "Nazwa pliku STL, STEP lub 3MF")),
    request_body(content = super::docs::BinaryFile, content_type = "application/octet-stream"),
    responses((status = 200, description = "Analiza geometrii", body = ModelAnalysis),
        (status = 422, description = "Nieprawidłowy lub zbyt złożony model", body = Value)))]
async fn analyze(headers: HeaderMap, input: Bytes) -> Result<Json<ModelAnalysis>, Error> {
    let filename = headers
        .get("x-model-filename")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let extension = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if filename.len() > 1024
        || !matches!(extension.as_str(), "stl" | "step" | "stp" | "3mf")
        || input.is_empty()
    {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Wybierz niepusty model STL, STEP lub 3MF.",
        ));
    }
    let _permit = BUSY.try_acquire().map_err(|_| {
        error(
            StatusCode::TOO_MANY_REQUESTS,
            "Trwa analiza innego modelu. Spróbuj za chwilę.",
        )
    })?;
    let workspace =
        Workspace(std::env::temp_dir().join(format!("rapidmaker-model-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&workspace.0).map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Nie można przygotować analizy.",
        )
    })?;
    fs::set_permissions(&workspace.0, fs::Permissions::from_mode(0o700)).map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Nie można przygotować analizy.",
        )
    })?;
    let source = workspace.0.join(format!("model.{extension}"));
    fs::write(&source, input).map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Nie można zapisać modelu do analizy.",
        )
    })?;
    let output_path = workspace.0.join("result.json");
    let output = fs::File::create(&output_path).map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Nie można przygotować analizy.",
        )
    })?;
    let mut command = isolated_worker(
        &source,
        output,
        "--analyze-model-worker",
        80 * 1024 * 1024,
        None,
        None,
    )
    .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Brak procesu analizy."))?;
    let status = tokio::time::timeout(Duration::from_secs(30), command.status())
        .await
        .map_err(|_| {
            error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Model jest zbyt złożony. Analiza przekroczyła limit czasu.",
            )
        })?
        .map_err(|_| {
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Proces analizy modeli jest niedostępny.",
            )
        })?;
    if !status.success() {
        return Err(error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Nie udało się bezpiecznie przeanalizować modelu. Sprawdź plik lub zamów ocenę ręczną.",
        ));
    }
    if fs::metadata(&output_path)
        .map(|m| m.len() > 8192)
        .unwrap_or(true)
    {
        return Err(error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Nieprawidłowy wynik analizy modelu.",
        ));
    }
    let bytes = fs::read(output_path)
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Brak wyniku analizy."))?;
    let mut result: ModelAnalysis = serde_json::from_slice(&bytes).map_err(|_| error(StatusCode::UNPROCESSABLE_ENTITY, "Nieobsługiwana geometria lub uszkodzony model. Wyeksportuj poprawny STL, STEP albo 3MF."))?;
    result.id = format!("analyzed_{}", uuid::Uuid::new_v4());
    Ok(Json(result))
}

fn isolated_worker(
    source: &Path,
    output: fs::File,
    flag: &str,
    output_limit: u64,
    metadata: Option<&Path>,
    preview: Option<&Path>,
) -> Result<tokio::process::Command, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    // Fail closed if bubblewrap/namespaces are unavailable: the parser cannot
    // read host files, reach the network or leave child processes running.
    let extension = source
        .extension()
        .and_then(|v| v.to_str())
        .ok_or("Missing model extension")?;
    let isolated_source = format!("/model.{extension}");
    let mut command = tokio::process::Command::new("/usr/bin/prlimit");
    command
        .arg(format!("--fsize={output_limit}"))
        .args([
            "--as=2147483648",
            "--cpu=25",
            "--nofile=64",
            "--",
            "/usr/bin/bwrap",
            "--unshare-all",
            "--die-with-parent",
            "--new-session",
            "--cap-drop",
            "ALL",
            "--ro-bind",
            "/usr",
            "/usr",
            "--symlink",
            "usr/lib",
            "/lib",
            "--symlink",
            "usr/lib",
            "/lib64",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--ro-bind",
        ])
        .arg(executable)
        .arg("/worker")
        .arg("--ro-bind")
        .arg(source)
        .arg(&isolated_source);
    if let Some(metadata) = metadata {
        command.arg("--bind").arg(metadata).arg("/metadata.json");
    }
    if let Some(preview) = preview {
        command.arg("--bind").arg(preview).arg("/preview.glb");
    }
    command
        .args(["--remount-ro", "/", "/worker", flag])
        .arg(&isolated_source)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .current_dir(source.parent().ok_or("Missing model directory")?)
        .stdin(std::process::Stdio::null())
        .stdout(output)
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    Ok(command)
}

/// Convert only geometry in the same restricted parser sandbox as analysis.
/// The caller owns both paths; output must be a fresh file in its private workspace.
#[derive(Serialize, Deserialize)]
pub struct PreparedModel {
    pub dimensions_mm: [f64; 3],
}

pub async fn prepare_for_slicing(source: &Path, output: &Path) -> Result<PreparedModel, String> {
    let extension = source.extension().and_then(|v| v.to_str()).unwrap_or("");
    if !matches!(extension, "stl" | "step" | "stp" | "3mf") {
        return Err("Unsupported source format".into());
    }
    let length = fs::metadata(source).map_err(|e| e.to_string())?.len();
    if length == 0 || length > MAX_BYTES as u64 {
        return Err("Invalid model size".into());
    }
    let _permit = BUSY.try_acquire().map_err(|_| "Model parser busy")?;
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output)
        .map_err(|e| e.to_string())?;
    let preview = output.with_file_name("preview.glb");
    let mut preview_created = false;
    let result = async {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&preview)
            .map_err(|e| e.to_string())?;
        preview_created = true;
        let metadata = output.with_extension("metadata.json");
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&metadata)
            .map_err(|e| e.to_string())?;
        let mut command = isolated_worker(
            source,
            file,
            "--prepare-slicing-worker",
            50 * 1024 * 1024,
            Some(&metadata),
            Some(&preview),
        )?;
        let status = tokio::time::timeout(Duration::from_secs(30), command.status())
            .await
            .map_err(|_| "Model conversion timed out")?
            .map_err(|_| "Model conversion unavailable")?;
        if !status.success() {
            return Err(if status.code() == Some(65) {
                "Unsupported or invalid model geometry"
            } else {
                "Model conversion unavailable"
            }
            .into());
        }
        let length = fs::metadata(output).map_err(|e| e.to_string())?.len();
        if !(84..=50 * 1024 * 1024).contains(&length) {
            return Err("Invalid canonical mesh size".into());
        }
        let preview_length = fs::metadata(&preview).map_err(|e| e.to_string())?.len();
        if !(28..=MAX_PREVIEW_BYTES as u64).contains(&preview_length) {
            return Err("Invalid preview size".into());
        }
        let preview_bytes = fs::read(&preview).map_err(|e| e.to_string())?;
        let document = gltf::Gltf::from_slice(&preview_bytes).map_err(|_| "Invalid GLB preview")?;
        if preview_bytes.get(..4) != Some(b"glTF")
            || u32::from_le_bytes(
                preview_bytes[8..12]
                    .try_into()
                    .map_err(|_| "Invalid GLB header")?,
            ) as usize
                != preview_bytes.len()
            || document.blob.as_ref().is_none_or(Vec::is_empty)
            || document.document.buffers().count() != 1
            || document
                .document
                .buffers()
                .any(|buffer| !matches!(buffer.source(), gltf::buffer::Source::Bin))
            || document.document.images().next().is_some()
            || document.document.materials().next().is_some()
        {
            return Err("Preview must contain self-contained geometry".into());
        }
        let mut bytes = Vec::new();
        fs::File::open(&metadata)
            .map_err(|e| e.to_string())?
            .take(8193)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 8192 {
            return Err("Invalid canonical mesh metadata size".into());
        }
        let prepared: PreparedModel = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if prepared
            .dimensions_mm
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0)
        {
            return Err("Invalid canonical mesh dimensions".into());
        }
        Ok(prepared)
    }
    .await;
    let _ = fs::remove_file(output.with_extension("metadata.json"));
    if result.is_err() {
        if preview_created {
            let _ = fs::remove_file(&preview);
        }
        let _ = fs::remove_file(output);
    }
    result
}

/// Called before Tokio starts, so parser panics/resource exhaustion stay in this child.
pub fn worker() -> bool {
    let mut args = std::env::args();
    args.next();
    let mode = args.next().unwrap_or_default();
    if !matches!(
        mode.as_str(),
        "--analyze-model-worker" | "--prepare-slicing-worker"
    ) {
        return false;
    }
    let result = args
        .next()
        .ok_or_else(|| "Missing model path".to_string())
        .and_then(|filename| {
            if mode == "--prepare-slicing-worker" {
                if fs::metadata(&filename).map_err(|e| e.to_string())?.len() > MAX_BYTES as u64 {
                    return Err("Input too large".into());
                }
                let input = fs::read(&filename).map_err(|e| e.to_string())?;
                let output = source_to_production(
                    &filename,
                    &input,
                    &ConversionLimits {
                        max_input_bytes: MAX_BYTES,
                        max_output_glb_bytes: MAX_PREVIEW_BYTES,
                        max_parse_seconds: 25,
                        ..Default::default()
                    },
                    &ThreeMfLimits::default(),
                )
                .map_err(|e| e.to_string())?;
                let metadata = serde_json::to_vec(&PreparedModel {
                    dimensions_mm: output.dimensions_mm,
                })
                .map_err(|e| e.to_string())?;
                fs::write("/metadata.json", metadata).map_err(|e| e.to_string())?;
                fs::write("/preview.glb", output.glb).map_err(|e| e.to_string())?;
                Ok(output.stl)
            } else {
                serde_json::to_vec(&analyze_file(&filename)?).map_err(|e| e.to_string())
            }
        });
    if result
        .and_then(|bytes| {
            std::io::stdout()
                .write_all(&bytes)
                .map_err(|e| e.to_string())
        })
        .is_err()
    {
        std::process::exit(65);
    }
    true
}

fn analyze_file(filename: &str) -> Result<ModelAnalysis, String> {
    let input = fs::read(filename).map_err(|e| e.to_string())?;
    let limits = ConversionLimits {
        max_input_bytes: MAX_BYTES,
        max_parse_seconds: 25,
        ..Default::default()
    };
    let output = source_to_preview_glb(filename, &input, &limits, &ThreeMfLimits::default())
        .map_err(|e| e.to_string())?;
    let dimensions_mm = output.analysis.dimensions_mm.map(f64::from);
    if dimensions_mm.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return Err("Degenerate model".into());
    }
    let center = std::array::from_fn(|axis| {
        (f64::from(output.analysis.bounds_min_mm[axis])
            + f64::from(output.analysis.bounds_max_mm[axis]))
            / 2.0
    });
    let weight_g =
        solid_volume(&output.glb, center).map(|mm3| (mm3 / 1000.0 * 2.0).ceil().max(1.0));
    Ok(ModelAnalysis {
        id: String::new(), dimensions_mm, weight_g,
        geometry_source: "parsed_model".into(),
        weight_source: if weight_g.is_some() { "solid_upper_bound" } else { "unavailable" }.into(),
        reason: Some(if weight_g.is_some() {
            "Wymiary odczytane z modelu. Masa do dostawy oszacowana zachowawczo jak dla pełnej bryły przy 2 g/cm³; to nie wynik slicera. STL przyjmujemy w mm."
        } else {
            "Wymiary odczytane z modelu, ale siatka nie jest zamkniętą, zgodnie zorientowaną bryłą. Masa i dostawa wymagają oceny. STL przyjmujemy w mm."
        }.into()),
    })
}

/// Weld positions and require exactly two triangles per edge. Orient each closed
/// shell consistently through its edge graph; reject non-orientable or touching
/// non-manifold shells. Sum shell volumes separately so opposite winding cannot cancel.
fn solid_volume(glb: &[u8], center: [f64; 3]) -> Option<f64> {
    let document = gltf::Gltf::from_slice(glb).ok()?;
    let blob = document.blob.as_ref()?;
    let mut vertices: HashMap<[u32; 3], usize> = HashMap::new();
    let mut edges: HashMap<(usize, usize), (usize, i8, bool)> = HashMap::new();
    let mut triangles = Vec::new();
    let mut volumes = Vec::new();
    let mut neighbors: Vec<Vec<(usize, i8)>> = Vec::new();
    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                return None;
            }
            let reader =
                primitive.reader(|buffer| (buffer.index() == 0).then_some(blob.as_slice()));
            let positions: Vec<_> = reader.read_positions()?.collect();
            let indices: Vec<_> = reader.read_indices()?.into_u32().collect();
            if indices.len() % 3 != 0 {
                return None;
            }
            for triangle in indices.chunks_exact(3) {
                let triangle_id = triangles.len();
                neighbors.push(Vec::with_capacity(3));
                let mut ids = [0; 3];
                let mut points = [[0.0; 3]; 3];
                for (slot, index) in triangle.iter().enumerate() {
                    let point = *positions.get(*index as usize)?;
                    let key = point.map(|v| if v == 0.0 { 0 } else { v.to_bits() });
                    let next_vertex = vertices.len();
                    ids[slot] = *vertices.entry(key).or_insert(next_vertex);
                    points[slot] =
                        std::array::from_fn(|axis| f64::from(point[axis]) - center[axis]);
                }
                if ids[0] == ids[1] || ids[1] == ids[2] || ids[0] == ids[2] {
                    return None;
                }
                for (a, b) in [(ids[0], ids[1]), (ids[1], ids[2]), (ids[2], ids[0])] {
                    let (key, direction) = if a < b { ((a, b), 1) } else { ((b, a), -1) };
                    if let Some(edge) = edges.get_mut(&key) {
                        if edge.2 {
                            return None;
                        }
                        edge.2 = true;
                        let relation = -direction * edge.1;
                        neighbors[triangle_id].push((edge.0, relation));
                        neighbors[edge.0].push((triangle_id, relation));
                    } else {
                        edges.insert(key, (triangle_id, direction, false));
                    }
                }
                let [a, b, c] = points;
                let cross = [
                    b[1] * c[2] - b[2] * c[1],
                    b[2] * c[0] - b[0] * c[2],
                    b[0] * c[1] - b[1] * c[0],
                ];
                triangles.push(ids);
                volumes.push((a[0] * cross[0] + a[1] * cross[1] + a[2] * cross[2]) / 6.0);
            }
        }
    }
    if edges.is_empty() || edges.values().any(|edge| !edge.2) {
        return None;
    }
    let mut orientations = vec![0i8; triangles.len()];
    let mut total_volume = 0.0;
    for start in 0..triangles.len() {
        if orientations[start] != 0 {
            continue;
        }
        let mut queue = vec![start];
        orientations[start] = 1;
        let mut volume = 0.0;
        while let Some(triangle) = queue.pop() {
            // Summing absolute tetrahedra gives a conservative volume bound
            // even for concave shells, without an expensive intersection solver.
            volume += volumes[triangle].abs();
            for &(neighbor, relation) in &neighbors[triangle] {
                let expected = orientations[triangle] * relation;
                if orientations[neighbor] == 0 {
                    orientations[neighbor] = expected;
                    queue.push(neighbor);
                } else if orientations[neighbor] != expected {
                    return None;
                }
            }
        }
        if !volume.is_finite() || volume.abs() < 1e-9 {
            return None;
        }
        total_volume += volume.abs();
    }
    // A closed edge graph can still have a pinched/bow-tie vertex. Its local
    // triangle fan must be one connected cycle, rather than several shells.
    let mut incident = vec![(0usize, 0usize); vertices.len()];
    for (triangle, ids) in triangles.iter().enumerate() {
        for &vertex in ids {
            incident[vertex].0 = triangle;
            incident[vertex].1 += 1;
        }
    }
    let mut visited = vec![usize::MAX; triangles.len()];
    for (vertex, &(start, expected)) in incident.iter().enumerate() {
        let mut queue = vec![start];
        let mut count = 0;
        visited[start] = vertex;
        while let Some(triangle) = queue.pop() {
            count += 1;
            for &(neighbor, _) in &neighbors[triangle] {
                if visited[neighbor] != vertex && triangles[neighbor].contains(&vertex) {
                    visited[neighbor] = vertex;
                    queue.push(neighbor);
                }
            }
        }
        if count != expected {
            return None;
        }
    }
    (total_volume.is_finite() && total_volume > 0.0).then_some(total_volume)
}
