//! Isolated, measured P1P estimates from trusted Bambu Studio presets.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    fmt, fs,
    io::Read,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::LazyLock,
    time::Duration,
};
use tokio::sync::Semaphore;

static BUSY: Semaphore = Semaphore::const_new(1);
const PROFILES: &str = include_str!("../../../data/bambu/profiles.json");
pub const PROFILE_VERSION: &str = "02.08.00.06-p1p-hardened-0.4-pei-supports1-orient1-fit2";

// Profile availability is separate from the operator's hardware approval.
static MATERIAL_PROFILES: LazyLock<Result<BTreeMap<String, String>, String>> =
    LazyLock::new(|| {
        let catalog: Value = serde_json::from_str(include_str!("../../../data/bambu/catalog.json"))
            .map_err(|e| e.to_string())?;
        let materials = catalog["materials"]
            .as_array()
            .ok_or("Invalid trusted catalog")?;
        Ok(materials
            .iter()
            .filter_map(|material| {
                if material["name"].as_str()?.starts_with("Bambu Support") {
                    return None;
                }
                Some((
                    material["id"].as_str()?.to_owned(),
                    material["p1p_04_profiles"]
                        .as_array()?
                        .first()?
                        .as_str()?
                        .to_owned(),
                ))
            })
            .collect())
    });

pub fn material_supported(id: &str) -> bool {
    MATERIAL_PROFILES
        .as_ref()
        .is_ok_and(|profiles| profiles.contains_key(id))
}

/// Conservative orthogonal fit before Bambu chooses the actual orientation and arrangement.
/// Limits match the trusted P1P snapshot: 256x256, Z=250 and an excluded 18x28 corner.
#[derive(Debug, PartialEq, Eq)]
pub enum ModelFitError {
    InvalidDimensions,
    SizeLimit,
    PrinterFit,
}
pub fn check_model_dimensions([x, y, z]: [f64; 3]) -> Result<(), ModelFitError> {
    if [x, y, z].iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return Err(ModelFitError::InvalidDimensions);
    }
    if [x, y, z].iter().any(|v| *v > 500.0) {
        return Err(ModelFitError::SizeLimit);
    }
    if [[x, y, z], [x, z, y], [y, z, x]]
        .iter()
        .any(|[a, b, h]| *h <= 250.0 && a.max(*b) <= 256.0 && a.min(*b) <= 238.0)
    {
        Ok(())
    } else {
        Err(ModelFitError::PrinterFit)
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Draft,
    #[default]
    Standard,
    Fine,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub quantity: u32,
    pub quality: Quality,
    pub wall_loops: u8,
    pub infill_percent: u8,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            quantity: 1,
            quality: Quality::Standard,
            wall_loops: 2,
            infill_percent: 15,
        }
    }
}
impl Options {
    pub fn validate(&self) -> Result<(), Error> {
        if !(1..=1000).contains(&self.quantity)
            || !(2..=8).contains(&self.wall_loops)
            || !(5..=100).contains(&self.infill_percent)
        {
            return Err(Error::InvalidOptions);
        }
        Ok(())
    }
    fn process_name(&self) -> &'static str {
        match self.quality {
            Quality::Draft => "0.28mm Extra Draft @BBL P1P",
            Quality::Standard => "0.20mm Standard @BBL P1P",
            Quality::Fine => "0.12mm Fine @BBL P1P",
        }
    }
    fn version(&self) -> String {
        format!(
            "{PROFILE_VERSION}-{}-walls{}-infill{}",
            self.process_name(),
            self.wall_loops,
            self.infill_percent
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlateBatch {
    pub copies_per_plate: u32,
    pub repeat_count: u32,
    pub time_seconds: u64,
    pub filament_grams: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchEstimate {
    pub quantity: u32,
    pub plates: u32,
    pub time_seconds: u64,
    pub filament_grams: f64,
    pub profile_version: String,
    pub batches: Vec<PlateBatch>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Estimate {
    pub time_seconds: u64,
    pub filament_grams: f64,
    pub profile_version: String,
}

#[derive(Debug)]
pub enum Error {
    Busy,
    UnsupportedMaterial,
    InvalidModel,
    InvalidOptions,
    Failed(String),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => f.write_str("Slicer is busy; retry later"),
            Self::UnsupportedMaterial => {
                f.write_str("Material is not qualified for P1P automatic slicing")
            }
            Self::InvalidOptions => {
                f.write_str("Expected quantity 1..1000, walls 2..8, infill 5..100 percent")
            }
            Self::InvalidModel => f.write_str("Expected a nonempty STL up to 64 MiB"),
            Self::Failed(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Failed(format!("Slicer I/O: {error}"))
    }
}
impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Failed(format!("Slicer JSON: {error}"))
    }
}

struct Workspace(PathBuf);
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn resolve(
    name: &str,
    profiles: &BTreeMap<&str, &Value>,
    depth: usize,
) -> Result<Map<String, Value>, Error> {
    if depth > 16 {
        return Err(Error::Failed(
            "Preset inheritance cycle or excessive depth".into(),
        ));
    }
    let source = profiles
        .get(name)
        .and_then(|v| v.as_object())
        .ok_or_else(|| Error::Failed(format!("Missing trusted preset: {name}")))?;
    let mut result = if let Some(parent) = source.get("inherits").and_then(Value::as_str) {
        resolve(parent, profiles, depth + 1)?
    } else {
        Map::new()
    };
    if let Some(includes) = source.get("include").and_then(Value::as_array) {
        for include in includes {
            let include = include
                .as_str()
                .ok_or_else(|| Error::Failed("Invalid preset include".into()))?;
            result.extend(resolve(include, profiles, depth + 1)?);
        }
    }
    result.extend(source.clone());
    result.remove("inherits");
    result.remove("include");
    Ok(result)
}

fn write_profiles(directory: &Path, filament: &str, options: &Options) -> Result<(), Error> {
    let snapshot: BTreeMap<String, Value> = serde_json::from_str(PROFILES)?;
    let profiles = snapshot
        .values()
        .filter_map(|v| Some((v.get("name")?.as_str()?, v)))
        .collect();
    for (file, name) in [
        ("machine", "Bambu Lab P1P 0.4 nozzle"),
        ("process", options.process_name()),
        ("filament", filament),
    ] {
        let mut profile = resolve(name, &profiles, 0)?;
        if file == "machine" {
            profile.insert("nozzle_type".into(), serde_json::json!(["hardened_steel"]));
        }
        if file == "process" {
            // The UI promises automatic supports; include their real time and material.
            profile.insert("enable_support".into(), Value::String("1".into()));
            // Bambu rejects the standard grid pattern at 100% density.
            if options.infill_percent == 100 {
                profile.insert(
                    "sparse_infill_pattern".into(),
                    Value::String("alignedrectilinear".into()),
                );
            }
            profile.insert(
                "wall_loops".into(),
                Value::String(options.wall_loops.to_string()),
            );
            profile.insert(
                "sparse_infill_density".into(),
                Value::String(format!("{}%", options.infill_percent)),
            );
            profile.insert(
                "curr_bed_type".into(),
                Value::String("Textured PEI Plate".into()),
            );
        }
        fs::write(
            directory.join(format!("{file}.json")),
            serde_json::to_vec(&profile)?,
        )?;
    }
    Ok(())
}

/// Only server-owned STL paths are accepted. No project settings or G-code from uploads
/// reach the slicer. The caller must enforce measured geometry, approved profiles and explicit purchase prices.
pub async fn slice(model_path: &Path, material_id: &str) -> Result<Estimate, Error> {
    let result = slice_job(model_path, material_id, &Options::default()).await?;
    Ok(Estimate {
        time_seconds: result.time_seconds,
        filament_grams: result.filament_grams,
        profile_version: result.profile_version,
    })
}

/// Arrange actual copies on the first plate, measure it, then reuse identical full
/// plates and slice the remainder. Bambu limits cloning to 512 copies per run; this
/// is a bounded feasible plan, not a guarantee of globally optimal plate count.
#[tracing::instrument(skip_all, level = "debug")]
pub async fn slice_job(
    model_path: &Path,
    material_id: &str,
    options: &Options,
) -> Result<BatchEstimate, Error> {
    let started = std::time::Instant::now();
    let result = async {
        options.validate()?;
        let filament = MATERIAL_PROFILES
            .as_ref()
            .map_err(|_| Error::Failed("Invalid trusted material catalog".into()))?
            .get(material_id)
            .ok_or(Error::UnsupportedMaterial)?;
        let _permit = BUSY.try_acquire().map_err(|_| Error::Busy)?;
        let metadata = fs::symlink_metadata(model_path)?;
        if !metadata.is_file()
            || metadata.len() == 0
            || metadata.len() > 64 * 1024 * 1024
            || model_path
                .extension()
                .and_then(|s| s.to_str())
                .is_none_or(|s| !s.eq_ignore_ascii_case("stl"))
        {
            return Err(Error::InvalidModel);
        }
        let workspace = Workspace(
            std::env::temp_dir().join(format!("rapidmaker-slicer-{}", uuid::Uuid::new_v4())),
        );
        fs::create_dir(&workspace.0)?;
        fs::set_permissions(&workspace.0, fs::Permissions::from_mode(0o700))?;
        // Copy into our private directory, avoiding changes to the input during execution.
        let mut input = Vec::with_capacity(metadata.len() as usize);
        fs::File::open(model_path)?
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut input)?;
        prepare_stl(&mut input)?;
        fs::write(workspace.0.join("model.stl"), input)?;
        write_profiles(&workspace.0, filament, options)?;
        let mut batches = Vec::with_capacity(2);
        let first = run_plate(&workspace, options.quantity.min(512)).await?;
        let capacity = first.copies_per_plate;
        let repeats = options.quantity / capacity;
        batches.push(PlateBatch {
            repeat_count: repeats,
            ..first
        });
        let remainder = options.quantity % capacity;
        if remainder > 0 {
            let remaining = run_plate(&workspace, remainder).await?;
            if remaining.copies_per_plate != remainder {
                return Err(Error::Failed(
                    "Remainder did not fit its measured plate".into(),
                ));
            }
            batches.push(remaining);
        }
        Ok(BatchEstimate {
            quantity: options.quantity,
            plates: batches.iter().map(|b| b.repeat_count).sum(),
            time_seconds: batches
                .iter()
                .map(|b| b.time_seconds * u64::from(b.repeat_count))
                .sum(),
            filament_grams: batches
                .iter()
                .map(|b| b.filament_grams * f64::from(b.repeat_count))
                .sum(),
            profile_version: options.version(),
            batches,
        })
    }
    .await;
    if let Err(error) = &result {
        let reason = match error {
            Error::Busy => "busy",
            Error::UnsupportedMaterial => "unsupported_material",
            Error::InvalidModel => "invalid_model",
            Error::InvalidOptions => "invalid_options",
            Error::Failed(_) => "slicer_failed",
        };
        tracing::warn!(
            event = "operation_failed",
            provider = "local",
            operation = "slice_job",
            elapsed_ms = started.elapsed().as_millis() as u64,
            reason
        );
    } else {
        tracing::debug!(
            event = "operation_completed",
            provider = "local",
            operation = "slice_job",
            elapsed_ms = started.elapsed().as_millis() as u64
        );
    }
    result
}

// Existing uploads can contain zero normals. Prepare only the private slicing copy,
// retaining the customer's source/preview coordinates and replacing no printer settings.
fn prepare_stl(bytes: &mut [u8]) -> Result<(), Error> {
    if bytes.len() < 84 || bytes.len() > 64 * 1024 * 1024 {
        return Err(Error::InvalidModel);
    }
    let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    if count == 0 || count.checked_mul(50).and_then(|v| v.checked_add(84)) != Some(bytes.len()) {
        return Err(Error::InvalidModel);
    }
    let mut min_z = f64::INFINITY;
    for facet in bytes[84..].chunks_exact_mut(50) {
        let p: [[f64; 3]; 3] = std::array::from_fn(|v| {
            std::array::from_fn(|axis| {
                let i = 12 + v * 12 + axis * 4;
                f64::from(f32::from_le_bytes([
                    facet[i],
                    facet[i + 1],
                    facet[i + 2],
                    facet[i + 3],
                ]))
            })
        });
        if p.iter()
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
        {
            return Err(Error::InvalidModel);
        }
        min_z = p.iter().fold(min_z, |z, vertex| z.min(vertex[2]));
        let u: [f64; 3] = std::array::from_fn(|i| p[1][i] - p[0][i]);
        let v: [f64; 3] = std::array::from_fn(|i| p[2][i] - p[0][i]);
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let length = n[0].hypot(n[1]).hypot(n[2]);
        for (out, normal) in facet[..12].chunks_exact_mut(4).zip(n) {
            out.copy_from_slice(
                &(if length == 0.0 { 0.0 } else { normal / length } as f32).to_le_bytes(),
            );
        }
    }
    // All vertices move together; the millimetre shape, scale and winding are unchanged.
    for facet in bytes[84..].chunks_exact_mut(50) {
        for i in [20, 32, 44] {
            let z = f32::from_le_bytes([facet[i], facet[i + 1], facet[i + 2], facet[i + 3]]);
            facet[i..i + 4].copy_from_slice(&((f64::from(z) - min_z) as f32).to_le_bytes());
        }
    }
    Ok(())
}

async fn run_plate(workspace: &Workspace, copies: u32) -> Result<PlateBatch, Error> {
    let out = workspace.0.join("out");
    if out.exists() {
        fs::remove_dir_all(&out)?;
    }
    fs::create_dir(&out)?;
    let mut command = tokio::process::Command::new("/usr/bin/prlimit");
    command.args([
        "--as=4294967296",
        "--cpu=60",
        "--fsize=134217728",
        "--nofile=128",
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
        "--ro-bind",
        "/opt/bambustudio-bin",
        "/opt/bambustudio-bin",
        "--symlink",
        "usr/lib",
        "/lib",
        "--symlink",
        "usr/lib",
        "/lib64",
        "--symlink",
        "usr/bin",
        "/bin",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--size",
        "134217728",
        "--tmpfs",
        "/tmp",
        "--dir",
        "/home",
        "--setenv",
        "HOME",
        "/home",
    ]);
    for file in ["model.stl", "machine.json", "process.json", "filament.json"] {
        command
            .arg("--ro-bind")
            .arg(workspace.0.join(file))
            .arg(format!("/{file}"));
    }
    command
        .arg("--bind")
        .arg(&out)
        .arg("/out")
        .args([
            "--chdir",
            "/out",
            "--remount-ro",
            "/",
            "--",
            "/opt/bambustudio-bin/AppRun",
            "--datadir",
            "/tmp/config",
            "--load-settings",
            "/machine.json;/process.json",
            "--load-filaments",
            "/filament.json",
            "--slice",
            "1",
            "--orient",
            "1",
            "--ensure-on-bed",
            "--arrange",
            "1",
            "--allow-rotations",
            "--clone-objects",
        ])
        .arg(copies.to_string())
        .args([
            "--mstpp",
            "55",
            "--mtcpp",
            "1000000",
            "--outputdir",
            "/out",
            "/model.stl",
        ])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let status = tokio::time::timeout(Duration::from_secs(65), command.status())
        .await
        .map_err(|_| Error::Failed("Slicing exceeded 65 seconds".into()))??;
    let result = out.join("result.json");
    if !status.success() {
        tracing::warn!(event = "slicer_exit", exit_code = status.code());
        // Read only a bounded result from the now-stopped isolated process, never log its payload.
        if fs::symlink_metadata(&result).is_ok_and(|m| m.is_file() && m.len() <= 1024 * 1024)
            && let Ok(bytes) = fs::read(&result)
            && let Ok(value) = serde_json::from_slice::<Value>(&bytes)
            && value["return_code"].as_i64() == Some(-6)
        {
            return Err(Error::InvalidModel);
        }
        return Err(Error::Failed(
            "Isolated slicer failed; check model or runtime installation".into(),
        ));
    }
    let result_metadata = fs::symlink_metadata(&result)?;
    if !result_metadata.is_file() || result_metadata.len() > 1024 * 1024 {
        return Err(Error::Failed("Slicer result exceeded size limit".into()));
    }
    let plate = parse_result(&fs::read(&result)?)?;
    if plate.copies_per_plate == 0 || plate.copies_per_plate > copies {
        return Err(Error::Failed(
            "Slicer returned an invalid copy count".into(),
        ));
    }
    Ok(plate)
}

fn parse_result(bytes: &[u8]) -> Result<PlateBatch, Error> {
    #[derive(Deserialize)]
    struct ResultFile {
        return_code: u32,
        sliced_plates: Vec<Plate>,
    }
    #[derive(Deserialize)]
    struct Plate {
        total_predication: f64,
        filaments: Vec<Filament>,
        objects: Vec<Value>,
    }
    #[derive(Deserialize)]
    struct Filament {
        total_used_g: f64,
    }
    let result: ResultFile = serde_json::from_slice(bytes)?;
    if result.return_code != 0 || result.sliced_plates.len() != 1 {
        return Err(Error::Failed(
            "Slicer did not produce one successful plate".into(),
        ));
    }
    let plate = &result.sliced_plates[0];
    let grams: f64 = plate.filaments.iter().map(|f| f.total_used_g).sum();
    if !plate.total_predication.is_finite()
        || plate.total_predication <= 0.0
        || plate.total_predication > 30.0 * 86400.0
        || plate
            .filaments
            .iter()
            .any(|f| !f.total_used_g.is_finite() || f.total_used_g <= 0.0)
        || !grams.is_finite()
        || grams <= 0.0
        || grams > 10000.0
    {
        return Err(Error::Failed(
            "Slicer returned invalid time or filament consumption".into(),
        ));
    }
    Ok(PlateBatch {
        copies_per_plate: plate
            .objects
            .len()
            .try_into()
            .map_err(|_| Error::Failed("Too many sliced copies".into()))?,
        repeat_count: 1,
        time_seconds: plate.total_predication.ceil() as u64,
        filament_grams: grams,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_zero_normal_mesh_is_prepared_without_changing_its_shape() {
        let mut bytes = include_bytes!("../../geometry/tests/fixtures/cube_20mm.stl").to_vec();
        for facet in bytes[84..].chunks_exact_mut(50) {
            facet[..12].fill(0);
            for i in [20, 32, 44] {
                let z = f32::from_le_bytes(facet[i..i + 4].try_into().unwrap());
                facet[i..i + 4].copy_from_slice(&(z + 52.0).to_le_bytes());
            }
        }
        let original = bytes.clone();
        prepare_stl(&mut bytes).unwrap();
        for (old, new) in original[84..]
            .chunks_exact(50)
            .zip(bytes[84..].chunks_exact(50))
        {
            let n: Vec<f32> = new[..12]
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            assert!((n.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() < 0.00001);
            for i in [12, 24, 36] {
                assert_eq!(&old[i..i + 8], &new[i..i + 8]);
                let before = f32::from_le_bytes(old[i + 8..i + 12].try_into().unwrap());
                let after = f32::from_le_bytes(new[i + 8..i + 12].try_into().unwrap());
                assert_eq!(before - 42.0, after);
            }
        }
        bytes[96..100].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(matches!(prepare_stl(&mut bytes), Err(Error::InvalidModel)));
        assert!(matches!(
            prepare_stl(&mut [0; 84]),
            Err(Error::InvalidModel)
        ));
        assert!(matches!(
            prepare_stl(&mut [0; 83]),
            Err(Error::InvalidModel)
        ));
    }

    #[test]
    fn supports_trusted_printer_profiles_but_not_support_only_or_missing_profiles() {
        assert!(material_supported("GFA00"));
        assert!(material_supported("GFB00")); // ABS still requires operator hardware approval.
        for id in [
            "GFS00", "GFS01", "GFS02", "GFS03", "GFS05", "GFS06", "GFT02", "GFU04", "unknown",
        ] {
            assert!(!material_supported(id));
        }
    }
    #[test]
    fn measured_geometry_respects_size_limit_build_height_and_xy_rotation() {
        for dimensions in [
            [256.0, 238.0, 250.0],
            [238.0, 256.0, 250.0],
            [20.0; 3],
            [200.0, 10.0, 250.001],
            [256.0, 250.0, 238.0],
        ] {
            assert_eq!(check_model_dimensions(dimensions), Ok(()));
        }
        for axis in 0..3 {
            let mut dimensions = [1.0; 3];
            dimensions[axis] = 500.001;
            assert_eq!(
                check_model_dimensions(dimensions),
                Err(ModelFitError::SizeLimit)
            );
            dimensions[axis] = 500.0;
            assert_eq!(
                check_model_dimensions(dimensions),
                Err(ModelFitError::PrinterFit)
            );
        }
        for dimensions in [[256.0, 251.0, 239.0], [257.0, 257.0, 257.0]] {
            assert_eq!(
                check_model_dimensions(dimensions),
                Err(ModelFitError::PrinterFit)
            );
        }
        for dimensions in [
            [f64::NAN, 1.0, 1.0],
            [f64::INFINITY, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ] {
            assert_eq!(
                check_model_dimensions(dimensions),
                Err(ModelFitError::InvalidDimensions)
            );
        }
        let snapshot: BTreeMap<String, Value> = serde_json::from_str(PROFILES).unwrap();
        let profiles = snapshot
            .values()
            .filter_map(|v| Some((v.get("name")?.as_str()?, v)))
            .collect();
        let machine = resolve("Bambu Lab P1P 0.4 nozzle", &profiles, 0).unwrap();
        assert_eq!(machine["printable_height"], "250");
        assert_eq!(
            machine["printable_area"],
            serde_json::json!(["0x0", "256x0", "256x256", "0x256"])
        );
        assert_eq!(
            machine["bed_exclude_area"],
            serde_json::json!(["0x0", "18x0", "18x28", "0x28"])
        );
    }
    #[test]
    fn quoted_profiles_include_automatic_supports() {
        let workspace = Workspace(
            std::env::temp_dir().join(format!("rapidmaker-profile-test-{}", uuid::Uuid::new_v4())),
        );
        fs::create_dir(&workspace.0).unwrap();
        let filament = MATERIAL_PROFILES.as_ref().unwrap().get("GFA00").unwrap();
        write_profiles(&workspace.0, filament, &Options::default()).unwrap();
        let profile: Value =
            serde_json::from_slice(&fs::read(workspace.0.join("process.json")).unwrap()).unwrap();
        assert_eq!(profile["enable_support"], "1");
        assert_eq!(profile["support_type"], "tree(auto)");
        assert_eq!(profile["sparse_infill_pattern"], "grid");
    }

    #[test]
    fn rejects_failed_and_empty_results() {
        for json in [
            r#"{"return_code":1,"sliced_plates":[]}"#,
            r#"{"return_code":0,"sliced_plates":[{"total_predication":0,"filaments":[],"objects":[{}]}]}"#,
        ] {
            assert!(parse_result(json.as_bytes()).is_err());
        }
    }
    #[test]
    fn rejects_out_of_range_options() {
        for options in [
            Options {
                quantity: 0,
                ..Options::default()
            },
            Options {
                quantity: 1001,
                ..Options::default()
            },
            Options {
                wall_loops: 1,
                ..Options::default()
            },
            Options {
                wall_loops: 9,
                ..Options::default()
            },
            Options {
                infill_percent: 4,
                ..Options::default()
            },
            Options {
                infill_percent: 101,
                ..Options::default()
            },
        ] {
            assert!(matches!(options.validate(), Err(Error::InvalidOptions)));
        }
    }
    #[tokio::test]
    #[ignore = "Requires Bambu Studio/bubblewrap; optional SLICER_TEST_MODEL server-owned binary STL"]
    async fn real_model() {
        let model = std::env::var_os("SLICER_TEST_MODEL")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../geometry/tests/fixtures/cube_20mm.stl")
            });
        for material in ["GFA00", "GFG02"] {
            let result = slice_job(&model, material, &Options::default())
                .await
                .unwrap();
            eprintln!("{material}: {}", serde_json::to_string(&result).unwrap());
            assert_eq!(result.quantity, 1);
            assert_eq!(result.plates, 1);
            assert!(result.time_seconds > 0 && result.filament_grams > 0.0);
        }
        let workspace = Workspace(
            std::env::temp_dir().join(format!("rapidmaker-invalid-mesh-{}", uuid::Uuid::new_v4())),
        );
        fs::create_dir(&workspace.0).unwrap();
        let mut degenerate = include_bytes!("../../geometry/tests/fixtures/cube_20mm.stl").to_vec();
        degenerate[84..].fill(0);
        let invalid = workspace.0.join("invalid.stl");
        fs::write(&invalid, degenerate).unwrap();
        assert!(matches!(
            slice_job(&invalid, "GFA00", &Options::default()).await,
            Err(Error::InvalidModel)
        ));
    }

    #[tokio::test]
    #[ignore = "Requires locally installed Bambu Studio and Linux bubblewrap"]
    async fn real_cube() {
        let model =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../geometry/tests/fixtures/cube_20mm.stl");
        let single = slice(&model, "GFA00").await.unwrap();
        for quantity in [1, 2, 10, 100, 1000] {
            let options = Options {
                quantity,
                ..Options::default()
            };
            let result = slice_job(&model, "GFA00", &options).await.unwrap();
            eprintln!("q{quantity}: {}", serde_json::to_string(&result).unwrap());
            assert_eq!(
                result
                    .batches
                    .iter()
                    .map(|b| b.copies_per_plate * b.repeat_count)
                    .sum::<u32>(),
                quantity
            );
            assert!(result.plates >= 1 && result.plates <= quantity);
            if quantity > 1 {
                assert!(result.time_seconds < single.time_seconds * u64::from(quantity));
            }
        }
        let draft = slice_job(
            &model,
            "GFA00",
            &Options {
                quality: Quality::Draft,
                ..Options::default()
            },
        )
        .await
        .unwrap();
        let fine_strong = slice_job(
            &model,
            "GFA00",
            &Options {
                quality: Quality::Fine,
                wall_loops: 8,
                infill_percent: 100,
                ..Options::default()
            },
        )
        .await
        .unwrap();
        eprintln!(
            "draft: {}; fine strong: {}",
            serde_json::to_string(&draft).unwrap(),
            serde_json::to_string(&fine_strong).unwrap()
        );
        assert!(fine_strong.time_seconds > draft.time_seconds);
        assert!(fine_strong.filament_grams > draft.filament_grams);
        let petg = slice(&model, "GFG02").await.unwrap();
        assert!(petg.time_seconds > 60 && petg.filament_grams > 1.0);
    }
}
