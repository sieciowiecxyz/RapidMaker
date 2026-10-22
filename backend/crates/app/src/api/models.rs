//! Bounded geometry analysis in a disposable subprocess. The shipping mass is a
//! conservative solid estimate, never a slicer result or customer-provided value.
use axum::{
    Json, Router,
    body::Bytes,
    extract::DefaultBodyLimit,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use rapidmaker_geometry::{ConversionLimits, ThreeMfLimits, source_to_preview_glb};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_MODELS: usize = 128;
const MAX_AGE: Duration = Duration::from_secs(24 * 3600);
static BUSY: Semaphore = Semaphore::const_new(1);
static MODELS: OnceLock<Mutex<HashMap<String, (Instant, ModelAnalysis)>>> = OnceLock::new();

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelAnalysis {
    pub id: String,
    pub dimensions_mm: [f64; 3],
    pub weight_g: Option<f64>,
    pub geometry_source: String,
    pub weight_source: String,
    pub reason: Option<String>,
}

/// Only this server's geometry can authorize shipping for an uploaded model.
pub fn resolve(id: &str) -> Option<ModelAnalysis> {
    let mut models = MODELS.get_or_init(Default::default).lock().ok()?;
    models.retain(|_, (created, _)| created.elapsed() < MAX_AGE);
    models.get(id).map(|(_, analysis)| analysis.clone())
}

pub fn router() -> Router {
    Router::new()
        .route("/api/models/analyze", post(analyze))
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
    let executable = std::env::current_exe()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Brak procesu analizy."))?;
    // Fail closed if bubblewrap/namespaces are unavailable: the parser cannot
    // read host files, reach the network or leave child processes running.
    let isolated_source = format!("/model.{extension}");
    let mut command = tokio::process::Command::new("/usr/bin/prlimit");
    command
        .args([
            "--as=2147483648",
            "--cpu=25",
            "--fsize=83886080",
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
        .arg(&source)
        .arg(&isolated_source)
        .args(["--remount-ro", "/", "/worker", "--analyze-model-worker"])
        .arg(&isolated_source)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .current_dir(&workspace.0)
        .stdin(std::process::Stdio::null())
        .stdout(output)
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
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
    let mut models = MODELS.get_or_init(Default::default).lock().map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Nie można zachować analizy.",
        )
    })?;
    models.retain(|_, (created, _)| created.elapsed() < MAX_AGE);
    if models.len() >= MAX_MODELS {
        let oldest = models
            .iter()
            .min_by_key(|(_, (created, _))| *created)
            .map(|(id, _)| id.clone());
        if let Some(id) = oldest {
            models.remove(&id);
        }
    }
    models.insert(result.id.clone(), (Instant::now(), result.clone()));
    Ok(Json(result))
}

/// Called before Tokio starts, so parser panics/resource exhaustion stay in this child.
pub fn worker() -> bool {
    let mut args = std::env::args();
    args.next();
    if args.next().as_deref() != Some("--analyze-model-worker") {
        return false;
    }
    let result = args
        .next()
        .ok_or(())
        .and_then(|filename| analyze_file(&filename).map_err(|_| ()));
    match result {
        Ok(result) => {
            let _ = std::io::stdout().write_all(&serde_json::to_vec(&result).unwrap());
        }
        Err(_) => std::process::exit(1),
    }
    true
}

fn analyze_file(filename: &str) -> Result<ModelAnalysis, String> {
    let input = fs::read(filename).map_err(|e| e.to_string())?;
    if filename.ends_with(".step") || filename.ends_with(".stp") {
        // Legacy tessellation does not apply assembly instances or STEP units.
        // Accept a single explicit mm context only; never silently mis-scale a parcel.
        let text = std::str::from_utf8(&input).map_err(|_| "Invalid STEP")?;
        let compact: String = text
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_ascii_uppercase();
        let units: Vec<_> = compact
            .split(';')
            .filter(|entity| entity.contains("LENGTH_UNIT()"))
            .collect();
        if units.len() != 1
            || !units[0].contains("SI_UNIT(.MILLI.,.METRE.)")
            || [
                "MAPPED_ITEM(",
                "REPRESENTATION_RELATIONSHIP(",
                "CONTEXT_DEPENDENT_SHAPE_REPRESENTATION(",
                "CONVERSION_BASED_UNIT(",
            ]
            .iter()
            .any(|item| compact.contains(item))
        {
            return Err(
                "STEP assembly or ambiguous/unsupported units; export a single mm part".into(),
            );
        }
    }
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
