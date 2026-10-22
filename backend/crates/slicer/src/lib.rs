//! Isolated, measured P1P estimates from trusted Bambu Studio presets.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    fmt, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::sync::Semaphore;

static BUSY: Semaphore = Semaphore::const_new(1);
const PROFILES: &str = include_str!("../../../data/bambu/profiles.json");
pub const PROFILE_VERSION: &str = "02.08.00.06-p1p-hardened-0.4-pei-standard-0.20";

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
    Failed(String),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => f.write_str("Slicer is busy; retry later"),
            Self::UnsupportedMaterial => {
                f.write_str("Material is not qualified for P1P automatic slicing")
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

fn write_profiles(directory: &Path, filament: &str) -> Result<(), Error> {
    let snapshot: BTreeMap<String, Value> = serde_json::from_str(PROFILES)?;
    let profiles = snapshot
        .values()
        .filter_map(|v| Some((v.get("name")?.as_str()?, v)))
        .collect();
    for (file, name) in [
        ("machine", "Bambu Lab P1P 0.4 nozzle"),
        ("process", "0.20mm Standard @BBL P1P"),
        ("filament", filament),
    ] {
        let mut profile = resolve(name, &profiles, 0)?;
        if file == "machine" {
            profile.insert("nozzle_type".into(), serde_json::json!(["hardened_steel"]));
        }
        if file == "process" {
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
/// reach the slicer. The caller must separately enforce inventory and purchase prices.
pub async fn slice(model_path: &Path, material_id: &str) -> Result<Estimate, Error> {
    let filament = match material_id {
        "GFA00" => "Bambu PLA Basic @BBL P1P",
        "GFG02" => "Bambu PETG HF @BBL P1P 0.4 nozzle",
        _ => return Err(Error::UnsupportedMaterial),
    };
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
    let workspace =
        Workspace(std::env::temp_dir().join(format!("rapidmaker-slicer-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&workspace.0)?;
    fs::set_permissions(&workspace.0, fs::Permissions::from_mode(0o700))?;
    // Copy into our private directory, avoiding changes to the input during execution.
    fs::copy(model_path, workspace.0.join("model.stl"))?;
    write_profiles(&workspace.0, filament)?;
    let out = workspace.0.join("out");
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
            "0",
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
    if !status.success() {
        return Err(Error::Failed(
            "Isolated slicer failed; check model or runtime installation".into(),
        ));
    }
    let result = out.join("result.json");
    let result_metadata = fs::symlink_metadata(&result)?;
    if !result_metadata.is_file() || result_metadata.len() > 1024 * 1024 {
        return Err(Error::Failed("Slicer result exceeded size limit".into()));
    }
    parse_result(&fs::read(&result)?)
}

fn parse_result(bytes: &[u8]) -> Result<Estimate, Error> {
    #[derive(Deserialize)]
    struct ResultFile {
        return_code: u32,
        sliced_plates: Vec<Plate>,
    }
    #[derive(Deserialize)]
    struct Plate {
        total_predication: f64,
        filaments: Vec<Filament>,
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
    Ok(Estimate {
        time_seconds: plate.total_predication.ceil() as u64,
        filament_grams: grams,
        profile_version: PROFILE_VERSION.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_failed_and_empty_results() {
        for json in [
            r#"{"return_code":1,"sliced_plates":[]}"#,
            r#"{"return_code":0,"sliced_plates":[{"total_predication":0,"filaments":[]}]}"#,
        ] {
            assert!(parse_result(json.as_bytes()).is_err());
        }
    }
    #[tokio::test]
    #[ignore = "Requires locally installed Bambu Studio and Linux bubblewrap"]
    async fn real_cube() {
        let model =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../geometry/tests/fixtures/cube_20mm.stl");
        for material in ["GFA00", "GFG02"] {
            let result = slice(&model, material).await.unwrap();
            eprintln!("{material}: {}", serde_json::to_string(&result).unwrap());
            assert!(result.time_seconds > 60 && result.filament_grams > 1.0);
        }
    }
}
