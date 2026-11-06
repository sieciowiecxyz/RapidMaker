use std::{
    collections::HashMap,
    io::{Cursor, Read},
    time::{Duration, Instant},
};

use lib3mf::{Model as ThreeMfModel, ParserConfig, SpecConformance};
mod glb;
pub use glb::write_glb;
use glb::write_glb_bounded;
mod step;
mod three_mf_mirrors;

const MAX_CANONICAL_STL_BYTES: usize = 50 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFormat {
    StlBinary,
    StlAscii,
    ThreeMf,
    Step,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatSupportStatus {
    Unsupported,
    ManualReviewRequired,
    PreviewSupported,
    QuoteSupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatGateResult {
    pub source_format: SourceFormat,
    pub status: FormatSupportStatus,
    pub reason_code: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreeMfLimits {
    pub max_entry_count: usize,
    pub max_uncompressed_bytes: u64,
    pub max_entry_bytes: u64,
}

impl Default for ThreeMfLimits {
    fn default() -> Self {
        Self {
            max_entry_count: 512,
            max_uncompressed_bytes: 96 * 1024 * 1024,
            max_entry_bytes: 96 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversionLimits {
    pub max_input_bytes: usize,
    pub max_output_glb_bytes: usize,
    pub max_vertices: usize,
    pub max_triangles: usize,
    pub max_coordinate_abs_mm: f32,
    pub max_parse_seconds: u64,
}

impl Default for ConversionLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 50 * 1024 * 1024,
            max_output_glb_bytes: 25 * 1024 * 1024,
            max_vertices: 1_000_000,
            max_triangles: (MAX_CANONICAL_STL_BYTES - 84) / 50,
            max_coordinate_abs_mm: 1_000_000.0,
            max_parse_seconds: 30,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewAnalysis {
    pub source_format: SourceFormat,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub bounds_min_mm: [f32; 3],
    pub bounds_max_mm: [f32; 3],
    pub dimensions_mm: [f32; 3],
    pub output_glb_bytes: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewOutput {
    pub glb: Vec<u8>,
    pub analysis: PreviewAnalysis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshScene {
    pub positions_mm: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewError {
    InputTooLarge,
    OutputTooLarge,
    ParseTimeout,
    UnsupportedFormat,
    MalformedInput,
    NonManifoldMesh,
    EmptyMesh,
    NonFiniteCoordinate,
    CoordinateOutOfBounds,
    TooManyVertices,
    TooManyTriangles,
    UnsafeContainer,
    DuplicateContainerPath,
    TooManyContainerEntries,
    ContainerTooLarge,
    ExternalReferenceDisabled,
}

impl std::fmt::Display for PreviewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputTooLarge => f.write_str("input too large"),
            Self::OutputTooLarge => f.write_str("output GLB too large"),
            Self::ParseTimeout => f.write_str("parse timeout"),
            Self::UnsupportedFormat => f.write_str("unsupported source format"),
            Self::MalformedInput => f.write_str("malformed mesh input"),
            Self::NonManifoldMesh => f.write_str("non-manifold mesh"),
            Self::EmptyMesh => f.write_str("empty mesh"),
            Self::NonFiniteCoordinate => f.write_str("non-finite coordinate"),
            Self::CoordinateOutOfBounds => f.write_str("coordinate out of bounds"),
            Self::TooManyVertices => f.write_str("too many vertices"),
            Self::TooManyTriangles => f.write_str("too many triangles"),
            Self::UnsafeContainer => f.write_str("unsafe container"),
            Self::DuplicateContainerPath => f.write_str("duplicate container path"),
            Self::TooManyContainerEntries => f.write_str("too many container entries"),
            Self::ContainerTooLarge => f.write_str("container too large"),
            Self::ExternalReferenceDisabled => f.write_str("external references disabled"),
        }
    }
}

impl std::error::Error for PreviewError {}

fn classify_three_mf_error(error: lib3mf::Error) -> PreviewError {
    match error {
        lib3mf::Error::InvalidModel(message)
            if message.contains("Non-manifold edge (vertices ") =>
        {
            PreviewError::NonManifoldMesh
        }
        _ => PreviewError::MalformedInput,
    }
}

#[tracing::instrument(skip_all, level = "debug")]
pub fn source_to_preview_glb(
    source_filename: &str,
    input: &[u8],
    limits: &ConversionLimits,
    three_mf_limits: &ThreeMfLimits,
) -> Result<PreviewOutput, PreviewError> {
    let started = Instant::now();
    if input.len() > limits.max_input_bytes {
        return Err(PreviewError::InputTooLarge);
    }
    let gate = gate_source_format(source_filename, input, three_mf_limits)?;
    if gate.status == FormatSupportStatus::Unsupported {
        return Err(PreviewError::UnsupportedFormat);
    }
    let (scene, source_format) = scene_from_source(gate.source_format, input, limits, started)?;
    build_preview_output(scene, source_format, limits)
}

/// Canonical millimetre geometry only: no project settings, G-code or attachments.
/// STL is limited to 50 MiB; its accompanying preview uses the GLB byte budget.
#[tracing::instrument(skip_all, level = "debug")]
pub fn source_to_stl(
    filename: &str,
    input: &[u8],
    limits: &ConversionLimits,
    three_mf_limits: &ThreeMfLimits,
) -> Result<Vec<u8>, PreviewError> {
    source_to_production(filename, input, limits, three_mf_limits).map(|output| output.stl)
}

fn triangle_normal([a, b, c]: [[f32; 3]; 3]) -> [f32; 3] {
    let u: [f64; 3] = std::array::from_fn(|i| f64::from(b[i]) - f64::from(a[i]));
    let v: [f64; 3] = std::array::from_fn(|i| f64::from(c[i]) - f64::from(a[i]));
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let length = n[0].hypot(n[1]).hypot(n[2]);
    if length == 0.0 {
        [0.0; 3]
    } else {
        n.map(|value| (value / length) as f32)
    }
}

pub struct ProductionOutput {
    pub stl: Vec<u8>,
    pub glb: Vec<u8>,
    pub dimensions_mm: [f64; 3],
}

/// Return metadata from this same parse, so callers never reparse untrusted geometry.
#[tracing::instrument(skip_all, level = "debug")]
pub fn source_to_production(
    filename: &str,
    input: &[u8],
    limits: &ConversionLimits,
    three_mf_limits: &ThreeMfLimits,
) -> Result<ProductionOutput, PreviewError> {
    let started = std::time::Instant::now();
    let result = (|| {
        let started = Instant::now();
        if input.len() > limits.max_input_bytes {
            return Err(PreviewError::InputTooLarge);
        }
        let gate = gate_source_format(filename, input, three_mf_limits)?;
        if gate.status == FormatSupportStatus::Unsupported {
            return Err(PreviewError::UnsupportedFormat);
        }
        let format = gate.source_format;
        let (scene, format) = scene_from_source(format, input, limits, started)?;
        let analysis = analyze_scene(&scene, format, limits)?;
        if analysis
            .dimensions_mm
            .iter()
            .any(|dimension| *dimension <= 0.0)
        {
            return Err(PreviewError::MalformedInput);
        }
        let output_len = analysis
            .triangle_count
            .checked_mul(50)
            .and_then(|n| n.checked_add(84))
            .filter(|n| *n <= MAX_CANONICAL_STL_BYTES)
            .ok_or(PreviewError::OutputTooLarge)?;
        let count =
            u32::try_from(analysis.triangle_count).map_err(|_| PreviewError::TooManyTriangles)?;
        let mut output = Vec::with_capacity(output_len);
        output.resize(80, 0);
        output.extend_from_slice(&count.to_le_bytes());
        for triangle in scene.indices.chunks_exact(3) {
            check_timeout(started, limits)?;
            let positions = [triangle[0], triangle[1], triangle[2]]
                .map(|index| scene.positions_mm.get(index as usize).copied());
            let [Some(a), Some(b), Some(c)] = positions else {
                return Err(PreviewError::MalformedInput);
            };
            // Some slicers mistake zero-normal binary STL for ASCII, depending on its vertices.
            for coordinate in triangle_normal([a, b, c]) {
                output.extend_from_slice(&coordinate.to_le_bytes());
            }
            for position in [a, b, c] {
                for coordinate in position {
                    output.extend_from_slice(&coordinate.to_le_bytes());
                }
            }
            output.extend_from_slice(&[0; 2]);
        }
        // Convert CAD millimetres/Z-up into glTF metres/Y-up at the node, without
        // changing the canonical millimetre mesh used by the slicer.
        let glb = write_glb_bounded(
            &scene,
            &analysis,
            limits.max_output_glb_bytes.min(25 * 1024 * 1024),
            r#","scale":[0.001,0.001,0.001],"rotation":[-0.70710678,0,0,0.70710678]"#,
        )?;
        check_timeout(started, limits)?;
        Ok(ProductionOutput {
            stl: output,
            glb,
            dimensions_mm: analysis.dimensions_mm.map(f64::from),
        })
    })();
    if let Err(reason) = &result {
        tracing::warn!(
            event = "operation_failed",
            provider = "local",
            operation = "geometry_conversion",
            elapsed_ms = started.elapsed().as_millis() as u64,
            reason = %reason
        );
    } else {
        tracing::debug!(
            event = "operation_completed",
            provider = "local",
            operation = "geometry_conversion",
            elapsed_ms = started.elapsed().as_millis() as u64
        );
    }
    result
}

#[tracing::instrument(skip_all, level = "debug")]
pub fn stl_to_preview_glb(
    input: &[u8],
    limits: &ConversionLimits,
) -> Result<PreviewOutput, PreviewError> {
    let started = Instant::now();
    if input.len() > limits.max_input_bytes {
        return Err(PreviewError::InputTooLarge);
    }
    let (scene, source_format) = parse_stl(input, limits, started)?;
    build_preview_output(scene, source_format, limits)
}

fn build_preview_output(
    scene: MeshScene,
    source_format: SourceFormat,
    limits: &ConversionLimits,
) -> Result<PreviewOutput, PreviewError> {
    let mut analysis = analyze_scene(&scene, source_format, limits)?;
    let glb = write_glb_bounded(&scene, &analysis, limits.max_output_glb_bytes, "")?;
    if glb.len() > limits.max_output_glb_bytes {
        return Err(PreviewError::OutputTooLarge);
    }
    analysis.output_glb_bytes = glb.len();
    Ok(PreviewOutput { glb, analysis })
}

fn scene_from_source(
    source_format: SourceFormat,
    input: &[u8],
    limits: &ConversionLimits,
    started: Instant,
) -> Result<(MeshScene, SourceFormat), PreviewError> {
    match source_format {
        SourceFormat::StlBinary | SourceFormat::StlAscii => parse_stl(input, limits, started),
        SourceFormat::ThreeMf => {
            scene_from_3mf(input, limits, started).map(|scene| (scene, source_format))
        }
        SourceFormat::Step => {
            scene_from_step(input, limits, started).map(|scene| (scene, source_format))
        }
    }
}

pub fn gate_source_format(
    filename: &str,
    input: &[u8],
    limits: &ThreeMfLimits,
) -> Result<FormatGateResult, PreviewError> {
    let lower = filename.to_lowercase();
    if lower.ends_with(".stl") {
        return Ok(FormatGateResult {
            source_format: SourceFormat::StlBinary,
            status: FormatSupportStatus::PreviewSupported,
            reason_code: "STL_PREVIEW_SUPPORTED",
        });
    }
    if lower.ends_with(".3mf") {
        validate_3mf_container(input, limits)?;
        return Ok(FormatGateResult {
            source_format: SourceFormat::ThreeMf,
            status: FormatSupportStatus::ManualReviewRequired,
            reason_code: "THREEMF_SECURITY_GATE_PASSED_MANUAL_REVIEW",
        });
    }
    if lower.ends_with(".step") || lower.ends_with(".stp") {
        validate_step_part21(input)?;
        return Ok(FormatGateResult {
            source_format: SourceFormat::Step,
            status: FormatSupportStatus::ManualReviewRequired,
            reason_code: "STEP_MANUAL_REVIEW_REQUIRED",
        });
    }
    Ok(FormatGateResult {
        source_format: SourceFormat::StlAscii,
        status: FormatSupportStatus::Unsupported,
        reason_code: "UNSUPPORTED_FORMAT",
    })
}

fn validate_step_part21(input: &[u8]) -> Result<(), PreviewError> {
    if input.len() > 50 * 1024 * 1024 {
        return Err(PreviewError::InputTooLarge);
    }
    let text = std::str::from_utf8(input).map_err(|_| PreviewError::MalformedInput)?;
    let upper = text.to_ascii_uppercase();
    if !upper.contains("ISO-10303-21")
        || !upper.contains("HEADER;")
        || !upper.contains("DATA;")
        || !upper.contains("END-ISO-10303-21")
    {
        return Err(PreviewError::MalformedInput);
    }
    if upper.contains("EXTERNAL_FILE")
        || upper.contains("EXTERNAL_REFERENCE")
        || upper.contains("DOCUMENT_FILE")
        || upper.contains("HTTP://")
        || upper.contains("HTTPS://")
        || upper.contains("FILE://")
    {
        return Err(PreviewError::ExternalReferenceDisabled);
    }
    Ok(())
}

fn validate_3mf_container(input: &[u8], limits: &ThreeMfLimits) -> Result<(), PreviewError> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(input)).map_err(|_| PreviewError::MalformedInput)?;
    if archive.len() > limits.max_entry_count {
        return Err(PreviewError::TooManyContainerEntries);
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut total = 0_u64;
    let mut saw_model = false;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|_| PreviewError::MalformedInput)?;
        let name = entry.name().map_err(|_| PreviewError::UnsafeContainer)?;
        let name = if entry.is_dir() {
            name.trim_end_matches('/')
        } else {
            name.as_ref()
        };
        let normalized = normalize_container_path(name)?;
        if !seen.insert(normalized.clone()) {
            return Err(PreviewError::DuplicateContainerPath);
        }
        if entry.encrypted() {
            return Err(PreviewError::UnsafeContainer);
        }
        total = total
            .checked_add(entry.size())
            .ok_or(PreviewError::ContainerTooLarge)?;
        if entry.size() > limits.max_entry_bytes || total > limits.max_uncompressed_bytes {
            return Err(PreviewError::ContainerTooLarge);
        }
        saw_model |= normalized == "3D/3dmodel.model";
        if normalized.ends_with(".rels") {
            let mut text = String::new();
            (&mut entry)
                .take(limits.max_entry_bytes.saturating_add(1))
                .read_to_string(&mut text)
                .map_err(|_| PreviewError::MalformedInput)?;
            if text.len() as u64 > limits.max_entry_bytes {
                return Err(PreviewError::ContainerTooLarge);
            }
            inspect_relationships(&text)?;
        }
    }
    archive
        .validate_header_consistency()
        .map_err(|error| match error {
            zip::result::ZipError::InvalidArchive(message)
                if message.contains("duplicate file names") =>
            {
                PreviewError::DuplicateContainerPath
            }
            _ => PreviewError::MalformedInput,
        })?;
    if saw_model {
        Ok(())
    } else {
        Err(PreviewError::MalformedInput)
    }
}

fn normalize_container_path(name: &str) -> Result<String, PreviewError> {
    let rejected = name.starts_with('/')
        || name.starts_with('\\')
        || name.contains('\\')
        || name.contains("//")
        || name.contains(':')
        || name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
    if rejected {
        Err(PreviewError::UnsafeContainer)
    } else {
        Ok(name.to_string())
    }
}

fn inspect_relationships(text: &str) -> Result<(), PreviewError> {
    if text.contains("TargetMode=\"External\"")
        || text.contains("TargetMode='External'")
        || text.contains("Target=\"http://")
        || text.contains("Target='http://")
        || text.contains("Target=\"https://")
        || text.contains("Target='https://")
        || text.contains("Target=\"file://")
        || text.contains("Target='file://")
    {
        Err(PreviewError::ExternalReferenceDisabled)
    } else {
        Ok(())
    }
}

pub fn parse_stl(
    input: &[u8],
    limits: &ConversionLimits,
    started: Instant,
) -> Result<(MeshScene, SourceFormat), PreviewError> {
    if let Some(scene) = parse_binary_stl(input, limits, started)? {
        return Ok((scene, SourceFormat::StlBinary));
    }
    let text = std::str::from_utf8(input).map_err(|_| PreviewError::UnsupportedFormat)?;
    if !text.trim_start().starts_with("solid") {
        return Err(PreviewError::UnsupportedFormat);
    }
    Ok((
        parse_ascii_stl(text, limits, started)?,
        SourceFormat::StlAscii,
    ))
}

fn parse_binary_stl(
    input: &[u8],
    limits: &ConversionLimits,
    started: Instant,
) -> Result<Option<MeshScene>, PreviewError> {
    if input.len() < 84 {
        return Ok(None);
    }
    let triangle_count =
        u32::from_le_bytes(input[80..84].try_into().expect("slice length")) as usize;
    let Some(expected_len) = 84_usize.checked_add(triangle_count.saturating_mul(50)) else {
        return Err(PreviewError::MalformedInput);
    };
    if expected_len != input.len() {
        return Ok(None);
    }
    if triangle_count > limits.max_triangles {
        return Err(PreviewError::TooManyTriangles);
    }
    let mut builder = MeshSceneBuilder::default();
    let mut cursor = 84;
    for _ in 0..triangle_count {
        cursor += 12;
        let mut points = [[0.0; 3]; 3];
        for point in &mut points {
            *point = [
                read_f32_le(input, cursor)?,
                read_f32_le(input, cursor + 4)?,
                read_f32_le(input, cursor + 8)?,
            ];
            cursor += 12;
        }
        builder.push_triangle(points[0], points[1], points[2], limits, started)?;
        cursor += 2;
    }
    Ok(Some(builder.finish()?))
}

fn parse_ascii_stl(
    text: &str,
    limits: &ConversionLimits,
    started: Instant,
) -> Result<MeshScene, PreviewError> {
    let mut builder = MeshSceneBuilder::default();
    let mut triangle = [[0.0; 3]; 3];
    let mut pending = 0;
    let mut saw_facet = false;
    for line in text.lines() {
        check_timeout(started, limits)?;
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("facet") => saw_facet = true,
            Some("vertex") => {
                let x = parse_ascii_coord(parts.next())?;
                let y = parse_ascii_coord(parts.next())?;
                let z = parse_ascii_coord(parts.next())?;
                if parts.next().is_some() {
                    return Err(PreviewError::MalformedInput);
                }
                triangle[pending] = [x, y, z];
                pending += 1;
                if pending == 3 {
                    builder.push_triangle(
                        triangle[0],
                        triangle[1],
                        triangle[2],
                        limits,
                        started,
                    )?;
                    pending = 0;
                }
            }
            _ => {}
        }
    }
    if !saw_facet || builder.indices.is_empty() || pending != 0 {
        return Err(PreviewError::MalformedInput);
    }
    builder.finish()
}

const BAMBU_TRIANGLE_SETTINGS: &[&str] = &[
    "paint_supports",
    "paint_seam",
    "paint_color",
    "mmu_segmentation",
    "paint_fuzzy_skin",
    "face_property",
];

// Preserve complete package validation, normalizing only painting and mirror metadata.
// Paths and expanded ZIP sizes have already passed validate_3mf_container.
type GeometryPackage<'a> = (
    std::borrow::Cow<'a, [u8]>,
    HashMap<String, three_mf_mirrors::Mirrors>,
);

// An optional hint only: ambiguous/large relationships retain the full parser path.
fn canonical_root_relationship(source: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Option<()> {
    use quick_xml::{Reader, events::Event};
    let entry = source.by_name("_rels/.rels").ok()?;
    if entry.size() > 64 * 1024 {
        return None;
    }
    let mut bytes = Vec::new();
    entry.take(64 * 1024 + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 64 * 1024 {
        return None;
    }
    let mut reader = Reader::from_str(std::str::from_utf8(&bytes).ok()?);
    let mut models = 0;
    loop {
        match reader.read_event().ok()? {
            Event::Start(e) | Event::Empty(e) if e.name().as_ref().ends_with("Relationship") => {
                let attrs = e.attributes().collect::<Result<Vec<_>, _>>().ok()?;
                let get = |key: &str| attrs.iter().find(|a| a.key.as_ref() == key);
                if get("Type")?.value.as_ref() == lib3mf::opc::MODEL_REL_TYPE {
                    let target = get("Target")?.value.as_ref();
                    if target.strip_prefix('/').unwrap_or(target) != "3D/3dmodel.model" {
                        return None;
                    }
                    models += 1;
                }
            }
            Event::Eof => return (models == 1).then_some(()),
            _ => {}
        }
    }
}

fn geometry_package<'a>(
    input: &'a [u8],
    limits: &ConversionLimits,
    started: Instant,
) -> Result<GeometryPackage<'a>, PreviewError> {
    use std::{borrow::Cow, io::Write};
    let mut source =
        zip::ZipArchive::new(Cursor::new(input)).map_err(|_| PreviewError::MalformedInput)?;
    check_timeout(started, limits)?;
    // Reject bad small actual roots before normalizing potentially large secondary parts.
    if canonical_root_relationship(&mut source).is_some()
        && let Ok(mut root) = source.by_name("3D/3dmodel.model")
        && root.size() <= 64 * 1024
    {
        check_timeout(started, limits)?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut root)
            .take(64 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| PreviewError::MalformedInput)?;
        if bytes.len() <= 64 * 1024 {
            let xml = std::str::from_utf8(&bytes).map_err(|_| PreviewError::MalformedInput)?;
            let (normalized, _) = three_mf_mirrors::normalize(xml, limits, started)?;
            lib3mf::parser::parse_model_from_reader(
                Cursor::new(normalized.as_deref().unwrap_or(xml).as_bytes()),
                ParserConfig::with_all_extensions().with_spec_conformance(SpecConformance::Lenient),
            )
            .map_err(classify_three_mf_error)?;
        }
        check_timeout(started, limits)?;
    }
    let mut replacements = HashMap::new();
    let mut mirrors = HashMap::new();
    for index in 0..source.len() {
        check_timeout(started, limits)?;
        let mut entry = source
            .by_index(index)
            .map_err(|_| PreviewError::MalformedInput)?;
        if entry
            .name()
            .map_err(|_| PreviewError::UnsafeContainer)?
            .ends_with(".model")
        {
            let mut xml = String::new();
            entry
                .read_to_string(&mut xml)
                .map_err(|_| PreviewError::MalformedInput)?;
            let (normalized, saved) = three_mf_mirrors::normalize(&xml, limits, started)?;
            mirrors.insert(
                entry
                    .name()
                    .map_err(|_| PreviewError::UnsafeContainer)?
                    .into_owned(),
                saved,
            );
            if let Some(xml) = normalized {
                replacements.insert(index, xml);
            }
            check_timeout(started, limits)?;
        }
    }
    if replacements.is_empty() {
        return Ok((Cow::Borrowed(input), mirrors));
    }
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..source.len() {
        check_timeout(started, limits)?;
        let entry = source
            .by_index(index)
            .map_err(|_| PreviewError::MalformedInput)?;
        if let Some(xml) = replacements.remove(&index) {
            output
                .start_file(
                    entry
                        .name()
                        .map_err(|_| PreviewError::UnsafeContainer)?
                        .as_ref(),
                    zip::write::SimpleFileOptions::default(),
                )
                .map_err(|_| PreviewError::MalformedInput)?;
            output
                .write_all(xml.as_bytes())
                .map_err(|_| PreviewError::MalformedInput)?;
        } else {
            output
                .raw_copy_file(entry)
                .map_err(|_| PreviewError::MalformedInput)?;
        }
    }
    check_timeout(started, limits)?;
    output
        .finish()
        .map(|file| (Cow::Owned(file.into_inner()), mirrors))
        .map_err(|_| PreviewError::MalformedInput)
}

fn scene_from_3mf(
    input: &[u8],
    limits: &ConversionLimits,
    started: Instant,
) -> Result<MeshScene, PreviewError> {
    let (normalized, mut mirrors) = geometry_package(input, limits, started)?;
    let input = normalized.as_ref();
    let config =
        ParserConfig::with_all_extensions().with_spec_conformance(SpecConformance::Lenient);
    let mut model = ThreeMfModel::from_reader_with_config(Cursor::new(input), config)
        .map_err(classify_three_mf_error)?;
    mirrors
        .remove("3D/3dmodel.model")
        .ok_or(PreviewError::MalformedInput)?
        .restore(&mut model)?;
    // Production-extension parts live inside the same already-validated ZIP.
    // IDs are local to each .model file, not global across the package.
    let mut package = lib3mf::opc::Package::open_lenient(Cursor::new(input), true)
        .map_err(|_| PreviewError::MalformedInput)?;
    let mut models = HashMap::from([("3D/3dmodel.model".to_owned(), model)]);
    for path in package.file_names() {
        if path.ends_with(".model") && !models.contains_key(&path) {
            check_timeout(started, limits)?;
            let xml = package
                .get_file(&path)
                .map_err(|_| PreviewError::MalformedInput)?;
            let mut part = lib3mf::parser::parse_model_xml_with_config(
                &xml,
                ParserConfig::with_all_extensions().with_spec_conformance(SpecConformance::Lenient),
            )
            .map_err(classify_three_mf_error)?;
            mirrors
                .remove(&path)
                .ok_or(PreviewError::MalformedInput)?
                .restore(&mut part)?;
            models.insert(path, part);
        }
    }
    if !mirrors.is_empty() {
        return Err(PreviewError::MalformedInput);
    }
    let model = &models["3D/3dmodel.model"];
    let scale = three_mf_unit_scale_mm(&model.unit)?;
    let mut walker = ThreeMfObjectWalker {
        models: &models,
        stack: Vec::new(),
        builder: MeshSceneBuilder::default(),
        limits,
        started,
    };
    let root_transform = mat4_scale(scale);
    if model.build.items.is_empty() {
        for object in &model.resources.objects {
            walker.emit_object("3D/3dmodel.model", object.id, root_transform)?;
        }
    } else {
        for item in &model.build.items {
            let path = three_mf_part_path(item.production_path.as_deref(), "3D/3dmodel.model")?;
            let part = models.get(path).ok_or(PreviewError::MalformedInput)?;
            let transform = mat4_mul(
                root_transform,
                mat4_mul(
                    item.transform
                        .map(mat4_from_3mf_transform)
                        .unwrap_or_else(mat4_identity),
                    mat4_scale(three_mf_unit_scale_mm(&part.unit)? / scale),
                ),
            );
            walker.emit_object(path, item.objectid, transform)?;
        }
    }

    walker.finish()
}

fn scene_from_step(
    input: &[u8],
    limits: &ConversionLimits,
    started: Instant,
) -> Result<MeshScene, PreviewError> {
    // Legacy tessellation does not apply assembly instances or STEP units.
    // Accept a single explicit mm context only; never silently mis-scale geometry.
    let text = std::str::from_utf8(input).map_err(|_| PreviewError::MalformedInput)?;
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
        return Err(PreviewError::UnsupportedFormat);
    }
    let mesh = step::ffi::tessellate_step(input, limits.max_triangles, limits.max_vertices)
        .map_err(|_| PreviewError::MalformedInput)?;
    match mesh.status {
        step::ffi::StepStatus::Ok => (),
        step::ffi::StepStatus::TooManyTriangles => return Err(PreviewError::TooManyTriangles),
        step::ffi::StepStatus::TooManyVertices => return Err(PreviewError::TooManyVertices),
        step::ffi::StepStatus::Unsupported => return Err(PreviewError::UnsupportedFormat),
        _ => return Err(PreviewError::MalformedInput),
    }
    let mut builder = MeshSceneBuilder::default();
    for triangle in mesh.points.chunks_exact(3) {
        let points = [0, 1, 2].map(|i| point3_from_mm(triangle[i].x, triangle[i].y, triangle[i].z));
        builder.push_polygon(&points, limits, started)?;
    }
    builder.finish()
}

fn three_mf_part_path<'a>(
    path: Option<&'a str>,
    current: &'a str,
) -> Result<&'a str, PreviewError> {
    let Some(path) = path else {
        return Ok(current);
    };
    // A leading slash denotes the ZIP root in 3MF; it never names a host file.
    let path = path.strip_prefix('/').unwrap_or(path);
    normalize_container_path(path)?;
    if !path.ends_with(".model") {
        return Err(PreviewError::UnsafeContainer);
    }
    Ok(path)
}

struct ThreeMfObjectWalker<'a> {
    models: &'a HashMap<String, ThreeMfModel>,
    stack: Vec<(String, usize)>,
    builder: MeshSceneBuilder,
    limits: &'a ConversionLimits,
    started: Instant,
}

impl<'a> ThreeMfObjectWalker<'a> {
    fn emit_object(
        &mut self,
        path: &str,
        object_id: usize,
        transform: [[f32; 4]; 4],
    ) -> Result<(), PreviewError> {
        check_timeout(self.started, self.limits)?;
        if self.stack.len() >= 64
            || self
                .stack
                .iter()
                .any(|(p, id)| p == path && *id == object_id)
        {
            return Err(PreviewError::MalformedInput);
        }
        let model = self.models.get(path).ok_or(PreviewError::MalformedInput)?;
        let object = model
            .resources
            .objects
            .iter()
            .find(|object| object.id == object_id)
            .ok_or(PreviewError::MalformedInput)?;
        self.stack.push((path.to_owned(), object_id));
        if let Some(mesh) = &object.mesh {
            let mirrored = transform_determinant(transform)? < 0.0;
            for triangle in &mesh.triangles {
                check_timeout(self.started, self.limits)?;
                let a = mesh
                    .vertices
                    .get(triangle.v1)
                    .ok_or(PreviewError::MalformedInput)?;
                let b = mesh
                    .vertices
                    .get(triangle.v2)
                    .ok_or(PreviewError::MalformedInput)?;
                let c = mesh
                    .vertices
                    .get(triangle.v3)
                    .ok_or(PreviewError::MalformedInput)?;
                let [b, c] = if mirrored { [c, b] } else { [b, c] };
                emit_triangle_points(
                    point3_from_mm(a.x, a.y, a.z),
                    point3_from_mm(b.x, b.y, b.z),
                    point3_from_mm(c.x, c.y, c.z),
                    transform,
                    &mut self.builder,
                    self.limits,
                    self.started,
                )?;
            }
        }
        for component in &object.components {
            let component_transform = component
                .transform
                .map(mat4_from_3mf_transform)
                .unwrap_or_else(mat4_identity);
            let child_path = three_mf_part_path(
                component
                    .production
                    .as_ref()
                    .and_then(|production| production.path.as_deref()),
                path,
            )?;
            let child = self
                .models
                .get(child_path)
                .ok_or(PreviewError::MalformedInput)?;
            let units = mat4_scale(
                three_mf_unit_scale_mm(&child.unit)? / three_mf_unit_scale_mm(&model.unit)?,
            );
            self.emit_object(
                child_path,
                component.objectid,
                mat4_mul(transform, mat4_mul(component_transform, units)),
            )?;
        }
        self.stack.pop();
        Ok(())
    }

    fn finish(self) -> Result<MeshScene, PreviewError> {
        self.builder.finish()
    }
}

fn emit_triangle_points(
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    transform: [[f32; 4]; 4],
    builder: &mut MeshSceneBuilder,
    limits: &ConversionLimits,
    started: Instant,
) -> Result<(), PreviewError> {
    builder.push_triangle(
        transform_point(transform, a),
        transform_point(transform, b),
        transform_point(transform, c),
        limits,
        started,
    )
}

fn three_mf_unit_scale_mm(unit: &str) -> Result<f32, PreviewError> {
    Ok(match unit.to_ascii_lowercase().as_str() {
        "micron" | "micrometer" => 0.001,
        "millimeter" => 1.0,
        "centimeter" => 10.0,
        "inch" => 25.4,
        "foot" => 304.8,
        "meter" => 1000.0,
        _ => return Err(PreviewError::UnsupportedFormat),
    })
}

fn mat4_identity() -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn mat4_scale(scale: f32) -> [[f32; 4]; 4] {
    [
        [scale, 0.0, 0.0, 0.0],
        [0.0, scale, 0.0, 0.0],
        [0.0, 0.0, scale, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn transform_determinant(matrix: [[f32; 4]; 4]) -> Result<f64, PreviewError> {
    if matrix.iter().flatten().any(|v| !v.is_finite()) {
        return Err(PreviewError::NonFiniteCoordinate);
    }
    let [[a, d, g, _], [b, e, h, _], [c, f, i, _], _] = matrix.map(|row| row.map(f64::from));
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    if !det.is_finite() || det == 0.0 {
        return Err(PreviewError::MalformedInput);
    }
    Ok(det)
}

fn mat4_from_3mf_transform(transform: [f64; 12]) -> [[f32; 4]; 4] {
    let [m00, m01, m02, m10, m11, m12, m20, m21, m22, tx, ty, tz] = transform;
    [
        [m00 as f32, m10 as f32, m20 as f32, 0.0],
        [m01 as f32, m11 as f32, m21 as f32, 0.0],
        [m02 as f32, m12 as f32, m22 as f32, 0.0],
        [tx as f32, ty as f32, tz as f32, 1.0],
    ]
}

fn mat4_mul(lhs: [[f32; 4]; 4], rhs: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for column in 0..4 {
        for row in 0..4 {
            out[column][row] = lhs[0][row] * rhs[column][0]
                + lhs[1][row] * rhs[column][1]
                + lhs[2][row] * rhs[column][2]
                + lhs[3][row] * rhs[column][3];
        }
    }
    out
}

fn transform_point(matrix: [[f32; 4]; 4], point: [f32; 3]) -> [f32; 3] {
    [
        matrix[0][0] * point[0] + matrix[1][0] * point[1] + matrix[2][0] * point[2] + matrix[3][0],
        matrix[0][1] * point[0] + matrix[1][1] * point[1] + matrix[2][1] * point[2] + matrix[3][1],
        matrix[0][2] * point[0] + matrix[1][2] * point[1] + matrix[2][2] * point[2] + matrix[3][2],
    ]
}

fn point3_from_mm(x: f64, y: f64, z: f64) -> [f32; 3] {
    [x as f32, y as f32, z as f32]
}

#[derive(Default)]
struct MeshSceneBuilder {
    // Exact bits retain signed zero and never merge nearby but different coordinates.
    vertices: HashMap<[u32; 3], u32>,
    positions_mm: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl MeshSceneBuilder {
    fn push_polygon(
        &mut self,
        points: &[[f32; 3]],
        limits: &ConversionLimits,
        started: Instant,
    ) -> Result<(), PreviewError> {
        if points.len() < 3 {
            return Ok(());
        }
        for index in 1..points.len() - 1 {
            self.push_triangle(points[0], points[index], points[index + 1], limits, started)?;
        }
        Ok(())
    }

    fn push_triangle(
        &mut self,
        a: [f32; 3],
        b: [f32; 3],
        c: [f32; 3],
        limits: &ConversionLimits,
        started: Instant,
    ) -> Result<(), PreviewError> {
        check_timeout(started, limits)?;
        if self.indices.len() / 3 >= limits.max_triangles {
            return Err(PreviewError::TooManyTriangles);
        }
        for point in [a, b, c] {
            if point.iter().any(|v| !v.is_finite()) {
                return Err(PreviewError::NonFiniteCoordinate);
            }
            if point.iter().any(|v| v.abs() > limits.max_coordinate_abs_mm) {
                return Err(PreviewError::CoordinateOutOfBounds);
            }
            let key = point.map(f32::to_bits);
            let index = if let Some(&index) = self.vertices.get(&key) {
                index
            } else {
                if self.positions_mm.len() >= limits.max_vertices {
                    return Err(PreviewError::TooManyVertices);
                }
                let index = u32::try_from(self.positions_mm.len())
                    .map_err(|_| PreviewError::TooManyVertices)?;
                self.positions_mm.push(point);
                self.vertices.insert(key, index);
                index
            };
            self.indices.push(index);
        }
        Ok(())
    }

    fn finish(self) -> Result<MeshScene, PreviewError> {
        if self.positions_mm.is_empty() || self.indices.is_empty() {
            return Err(PreviewError::EmptyMesh);
        }
        Ok(MeshScene {
            positions_mm: self.positions_mm,
            indices: self.indices,
        })
    }
}

fn analyze_scene(
    scene: &MeshScene,
    source_format: SourceFormat,
    limits: &ConversionLimits,
) -> Result<PreviewAnalysis, PreviewError> {
    if scene.positions_mm.is_empty() || scene.indices.is_empty() {
        return Err(PreviewError::EmptyMesh);
    }
    if scene.positions_mm.len() > limits.max_vertices {
        return Err(PreviewError::TooManyVertices);
    }
    let triangle_count = scene.indices.len() / 3;
    if triangle_count == 0 || !scene.indices.len().is_multiple_of(3) {
        return Err(PreviewError::MalformedInput);
    }
    if triangle_count > limits.max_triangles {
        return Err(PreviewError::TooManyTriangles);
    }

    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in &scene.positions_mm {
        for axis in 0..3 {
            let value = vertex[axis];
            if !value.is_finite() {
                return Err(PreviewError::NonFiniteCoordinate);
            }
            if value.abs() > limits.max_coordinate_abs_mm {
                return Err(PreviewError::CoordinateOutOfBounds);
            }
            min[axis] = min[axis].min(value);
            max[axis] = max[axis].max(value);
        }
    }
    let dimensions = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    if dimensions.iter().any(|value| !value.is_finite()) {
        return Err(PreviewError::NonFiniteCoordinate);
    }
    Ok(PreviewAnalysis {
        source_format,
        vertex_count: scene.positions_mm.len(),
        triangle_count,
        bounds_min_mm: min,
        bounds_max_mm: max,
        dimensions_mm: dimensions,
        output_glb_bytes: 0,
    })
}

fn read_f32_le(input: &[u8], offset: usize) -> Result<f32, PreviewError> {
    let bytes = input
        .get(offset..offset + 4)
        .ok_or(PreviewError::MalformedInput)?;
    Ok(f32::from_le_bytes(bytes.try_into().expect("slice length")))
}

fn parse_ascii_coord(value: Option<&str>) -> Result<f32, PreviewError> {
    value
        .ok_or(PreviewError::MalformedInput)?
        .parse::<f32>()
        .map_err(|_| PreviewError::MalformedInput)
}

fn check_timeout(started: Instant, limits: &ConversionLimits) -> Result<(), PreviewError> {
    if started.elapsed() > Duration::from_secs(limits.max_parse_seconds) {
        Err(PreviewError::ParseTimeout)
    } else {
        Ok(())
    }
}

fn pad_to_4(bytes: &mut Vec<u8>, pad_byte: u8) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(pad_byte);
    }
}

fn gltf_number(value: f32) -> String {
    if value == 0.0 {
        "0".to_string()
    } else {
        f64::from(value).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn default_triangle_limit_fills_canonical_stl_byte_budget() {
        let triangles = ConversionLimits::default().max_triangles;
        assert_eq!(triangles, 1_048_574);
        assert!(84 + triangles * 50 <= MAX_CANONICAL_STL_BYTES);
        assert!(84 + (triangles + 1) * 50 > MAX_CANONICAL_STL_BYTES);
    }

    const CUBE_BINARY: &[u8] = include_bytes!("../tests/fixtures/cube_20mm.stl");
    const CUBE_ASCII: &[u8] = include_bytes!("../tests/fixtures/cube_20mm_ascii.stl");
    const TINY_CUBE: &[u8] = include_bytes!("../tests/fixtures/tiny_cube_1mm.stl");
    const HUGE_CUBE: &[u8] = include_bytes!("../tests/fixtures/huge_cube_1000mm.stl");
    const OFF_CENTER_CUBE: &[u8] = include_bytes!("../tests/fixtures/off_center_cube.stl");
    const INVALID: &[u8] = include_bytes!("../tests/fixtures/invalid.stl");
    const MALFORMED: &[u8] = include_bytes!("../tests/fixtures/malformed_mesh.stl");
    const NAN_INF: &[u8] = include_bytes!("../tests/fixtures/nan_inf_coordinates.stl");
    const OBJ_SINGLE_CUBE: &[u8] = include_bytes!("../tests/fixtures/single_cube.obj");
    const PLY_SIMPLE: &[u8] = include_bytes!("../tests/fixtures/simple.ply");
    const GLB_SIMPLE: &[u8] = include_bytes!("../tests/fixtures/simple.glb");
    const THREEMF_SIMPLE: &[u8] = include_bytes!("../tests/fixtures/simple_model.3mf");
    const THREEMF_NON_MANIFOLD: &[u8] = include_bytes!("../tests/fixtures/non_manifold.3mf");
    const THREEMF_TRAVERSAL: &[u8] = include_bytes!("../tests/fixtures/zip_path_traversal.3mf");
    const THREEMF_DUPLICATE: &[u8] =
        include_bytes!("../tests/fixtures/zip_duplicate_normalized.3mf");
    const THREEMF_EXTERNAL_REL: &[u8] =
        include_bytes!("../tests/fixtures/zip_external_relationship.3mf");
    const THREEMF_BOMB: &[u8] = include_bytes!("../tests/fixtures/zip_bomb_limit.3mf");
    const GLTF_EXTERNAL: &[u8] = include_bytes!("../tests/fixtures/gltf_external_ref.gltf");
    const GLTF_EMBEDDED_TRIANGLE: &[u8] =
        include_bytes!("../tests/fixtures/gltf_embedded_triangle.gltf");
    const STEP_BLOCK: &[u8] = include_bytes!("../tests/fixtures/step_block.step");
    const STEP_CYLINDER: &[u8] = include_bytes!("../tests/fixtures/step_cylinder.step");
    const STEP_UNSUPPORTED_NURBS: &[u8] =
        include_bytes!("../tests/fixtures/step_unsupported_nurbs.step");

    #[test]
    fn three_mf_non_manifold_rejection_is_typed_without_relaxing_validation() {
        let limits = ConversionLimits::default();
        let archive_limits = ThreeMfLimits::default();
        // Padding bypasses the small-root preflight and exercises full SDK loading.
        // A bad vertex index remains MalformedInput rather than a topology error.
        for (padding, bad_index, expected) in [
            (0, false, PreviewError::NonManifoldMesh),
            (64 * 1024, false, PreviewError::NonManifoldMesh),
            (0, true, PreviewError::MalformedInput),
        ] {
            let mut source = zip::ZipArchive::new(Cursor::new(THREEMF_NON_MANIFOLD)).unwrap();
            let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
            for index in 0..source.len() {
                let mut entry = source.by_index(index).unwrap();
                if entry.name().unwrap() == "3D/3dmodel.model" {
                    let mut xml = String::new();
                    entry.read_to_string(&mut xml).unwrap();
                    if bad_index {
                        xml = xml.replace("v1=\"0\"", "v1=\"99\"");
                    }
                    xml.push_str(&" ".repeat(padding));
                    output
                        .start_file("3D/3dmodel.model", zip::write::SimpleFileOptions::default())
                        .unwrap();
                    output.write_all(xml.as_bytes()).unwrap();
                } else {
                    output.raw_copy_file(entry).unwrap();
                }
            }
            let input = output.finish().unwrap().into_inner();
            assert_eq!(
                source_to_preview_glb("mesh.3mf", &input, &limits, &archive_limits).unwrap_err(),
                expected
            );
            assert_eq!(
                source_to_production("mesh.3mf", &input, &limits, &archive_limits).err(),
                Some(expected)
            );
        }
    }

    #[test]
    fn unrelated_sdk_errors_remain_malformed() {
        for error in [
            lib3mf::Error::InvalidModel("bad vertex index".into()),
            lib3mf::Error::InvalidModel("non-finite coordinate".into()),
            lib3mf::Error::Io(std::io::Error::other("Non-manifold edge (vertices 0-1)")),
        ] {
            assert_eq!(classify_three_mf_error(error), PreviewError::MalformedInput);
        }
        assert_eq!(
            PreviewError::NonManifoldMesh.to_string(),
            "non-manifold mesh"
        );
    }

    fn mirror_package(build: &str, component: Option<&str>, extra: usize) -> Vec<u8> {
        let mesh = r#"<object id="1" type="model"><mesh><vertices>
          <vertex x="0" y="0" z="0"/><vertex x="2" y="0" z="0"/>
          <vertex x="0" y="2" z="0"/><vertex x="0" y="0" z="2"/>
          </vertices><triangles><triangle v1="0" v2="2" v3="1"/>
          <triangle v1="0" v2="1" v3="3"/><triangle v1="0" v2="3" v3="2"/>
          <triangle v1="1" v2="2" v3="3"/></triangles></mesh></object>"#;
        let (assembly, id) = match component {
            Some(transform) => (
                format!(
                    r#"<object id="2" type="model"><components><component objectid="1" transform="{transform}"/></components></object>"#
                ),
                2,
            ),
            None => (String::new(), 1),
        };
        let xml = format!(
            r#"<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"><resources>{mesh}{assembly}</resources><build><item objectid="{id}" transform="{build}"/></build></model>"#
        );
        let mut source = zip::ZipArchive::new(Cursor::new(THREEMF_SIMPLE)).unwrap();
        let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for index in 0..source.len() {
            let entry = source.by_index(index).unwrap();
            if entry.name().unwrap() == "3D/3dmodel.model" {
                output
                    .start_file("3D/3dmodel.model", zip::write::SimpleFileOptions::default())
                    .unwrap();
                output.write_all(xml.as_bytes()).unwrap();
            } else {
                output.raw_copy_file(entry).unwrap();
            }
        }
        for index in 0..extra {
            output
                .start_file(
                    format!("Metadata/extra-{index}"),
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
        }
        output.finish().unwrap().into_inner()
    }

    #[test]
    fn root_relationship_hint_skips_ambiguous_and_oversized_relationships() {
        let relationship = format!(
            r#"<Relationship Type="{}" Target="/3D/3dmodel.model"/>"#,
            lib3mf::opc::MODEL_REL_TYPE
        );
        for (relationships, expected) in [
            (relationship.clone(), true),
            (relationship.repeat(2), false),
            (format!("{relationship}{}", " ".repeat(64 * 1024)), false),
        ] {
            let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
            output
                .start_file("_rels/.rels", zip::write::SimpleFileOptions::default())
                .unwrap();
            write!(output, "<Relationships>{relationships}</Relationships>").unwrap();
            let input = output.finish().unwrap().into_inner();
            let mut source = zip::ZipArchive::new(Cursor::new(input.as_slice())).unwrap();
            assert_eq!(canonical_root_relationship(&mut source).is_some(), expected);
        }
    }

    #[test]
    fn alternate_actual_root_ignores_bad_metadata_in_unused_canonical_model() {
        let mut source = zip::ZipArchive::new(Cursor::new(THREEMF_SIMPLE)).unwrap();
        let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for index in 0..source.len() {
            let mut entry = source.by_index(index).unwrap();
            let name = entry.name().unwrap().into_owned();
            if name == "3D/3dmodel.model" || name == "_rels/.rels" {
                let mut xml = String::new();
                entry.read_to_string(&mut xml).unwrap();
                let replacement = if name == "3D/3dmodel.model" {
                    output.start_file("3D/actual.model", options).unwrap();
                    output.write_all(xml.as_bytes()).unwrap();
                    xml.replacen(
                        "<resources>",
                        r#"<metadata name="3DManufacturing:3mfVersion">1</metadata><resources>"#,
                        1,
                    )
                } else {
                    xml.replace("3D/3dmodel.model", "3D/actual.model")
                };
                output.start_file(name, options).unwrap();
                output.write_all(replacement.as_bytes()).unwrap();
            } else {
                output.raw_copy_file(entry).unwrap();
            }
        }
        let input = output.finish().unwrap().into_inner();
        let limits = ConversionLimits::default();
        assert!(geometry_package(&input, &limits, Instant::now()).is_ok());
        let baseline = source_to_production(
            "baseline.3mf",
            THREEMF_SIMPLE,
            &limits,
            &ThreeMfLimits::default(),
        )
        .unwrap();
        let result =
            source_to_production("alternate.3mf", &input, &limits, &ThreeMfLimits::default())
                .unwrap();
        assert_eq!(result.stl, baseline.stl);
    }

    #[test]
    fn small_root_preflight_rejects_metadata_and_skips_large_or_noncanonical_roots() {
        for metadata in [
            r#"<metadata name="3DManufacturing:3mfVersion">1</metadata>"#,
            r#"<metadata>1</metadata>"#,
        ] {
            for (root_path, padding, rejected) in [
                ("3D/3dmodel.model", 0, true),
                ("3D/3dmodel.model", 64 * 1024, false),
                ("3D/other.model", 0, false),
            ] {
                let mut source = zip::ZipArchive::new(Cursor::new(THREEMF_SIMPLE)).unwrap();
                let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
                for index in 0..source.len() {
                    let mut entry = source.by_index(index).unwrap();
                    if entry.name().unwrap() == "3D/3dmodel.model" {
                        let mut xml = String::new();
                        entry.read_to_string(&mut xml).unwrap();
                        let xml = xml.replacen(
                            "<resources>",
                            &format!("{metadata}{}<resources>", " ".repeat(padding)),
                            1,
                        );
                        assert!(xml.contains(metadata));
                        output
                            .start_file(root_path, zip::write::SimpleFileOptions::default())
                            .unwrap();
                        output.write_all(xml.as_bytes()).unwrap();
                    } else {
                        output.raw_copy_file(entry).unwrap();
                    }
                }
                let input = output.finish().unwrap().into_inner();
                let result = geometry_package(&input, &ConversionLimits::default(), Instant::now());
                assert_eq!(result.is_err(), rejected, "{root_path}, padding={padding}");
                if let Err(error) = result {
                    assert_eq!(error, PreviewError::MalformedInput);
                }
            }
        }
    }

    #[test]
    fn three_mf_mirrors_preserve_coordinates_bounds_and_positive_volume() {
        const IDENTITY: &str = "1 0 0 0 1 0 0 0 1 0 0 0";
        const MIRROR: &str = "-1 0 0 0 1 0 0 0 1 10 20 30";
        for (build, component, min, max) in [
            (MIRROR, None, [8., 20., 30.], [10., 22., 32.]),
            (IDENTITY, Some(MIRROR), [8., 20., 30.], [10., 22., 32.]),
            (MIRROR, Some(MIRROR), [0., 40., 60.], [2., 42., 62.]),
            // Actual case22 build matrix, applied to a small synthetic tetrahedron.
            (
                "-1 0 0 0 1 0 0 0 1 128.20001 -179.19999 17.5",
                None,
                [126.20001, -179.2, 17.5],
                [128.20001, -177.2, 19.5],
            ),
        ] {
            let input = mirror_package(build, component, 0);
            let output = source_to_production(
                "mirror.3mf",
                &input,
                &ConversionLimits::default(),
                &ThreeMfLimits::default(),
            )
            .unwrap();
            let (scene, _) =
                parse_stl(&output.stl, &ConversionLimits::default(), Instant::now()).unwrap();
            let analysis =
                analyze_scene(&scene, SourceFormat::ThreeMf, &ConversionLimits::default()).unwrap();
            assert_eq!(analysis.bounds_min_mm, min);
            assert_eq!(analysis.bounds_max_mm, max);
            assert_eq!(analysis.triangle_count, 4);
            assert!((signed_volume(&scene) - 8.0 / 6.0).abs() < 0.001);
            let mut expected = [[0., 0., 0.], [2., 0., 0.], [0., 2., 0.], [0., 0., 2.]];
            for transform in component.into_iter().chain([build]) {
                let matrix = transform
                    .split_whitespace()
                    .map(|v| v.parse().unwrap())
                    .collect::<Vec<f64>>();
                let matrix = mat4_from_3mf_transform(matrix.try_into().unwrap());
                expected = expected.map(|point| transform_point(matrix, point));
            }
            for point in expected {
                assert!(scene.positions_mm.contains(&point), "missing {point:?}");
            }
        }
    }

    #[test]
    fn three_mf_mirror_normalization_keeps_invalid_models_rejected() {
        const IDENTITY: &str = "1 0 0 0 1 0 0 0 1 0 0 0";
        for invalid in [
            "NaN 0 0 0 1 0 0 0 1 0 0 0",
            "-1 0 0 0 inf 0 0 0 1 0 0 0",
            "0 0 0 0 1 0 0 0 1 0 0 0",
            "-1 0 0 0 1 0 0 0 1 0 0",
        ] {
            for (build, component) in [(invalid, None), (IDENTITY, Some(invalid))] {
                assert!(
                    source_to_production(
                        "bad.3mf",
                        &mirror_package(build, component, 0),
                        &ConversionLimits::default(),
                        &ThreeMfLimits::default()
                    )
                    .is_err()
                );
            }
        }
        // Restoring must fail closed if the parser changes a count, ID or matrix.
        let xml = r#"<model><resources><object id="1"><components><component objectid="2"/></components></object></resources><build><item objectid="1" transform="-1 0 0 0 1 0 0 0 1 0 0 0"/></build></model>"#;
        for tamper in 0..3 {
            let (normalized, saved) =
                three_mf_mirrors::normalize(xml, &ConversionLimits::default(), Instant::now())
                    .unwrap();
            let mut model = lib3mf::parser::parse_model_xml_with_config(
                normalized.as_deref().unwrap(),
                ParserConfig::with_all_extensions(),
            )
            .unwrap();
            match tamper {
                0 => model.build.items.clear(),
                1 => model.resources.objects[0].id = 9,
                _ => model.build.items[0].transform = None,
            }
            assert!(saved.restore(&mut model).is_err());
        }
    }

    #[test]
    fn three_mf_small_component_scale_keeps_preexisting_valid_geometry() {
        let input = mirror_package(
            "1 0 0 0 1 0 0 0 1 0 0 0",
            Some("0.0001 0 0 0 0.0001 0 0 0 0.0001 0 0 0"),
            0,
        );
        let mut source = zip::ZipArchive::new(Cursor::new(input)).unwrap();
        let mut package = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for index in 0..source.len() {
            let mut entry = source.by_index(index).unwrap();
            if entry.name().unwrap() == "3D/3dmodel.model" {
                let mut xml = String::new();
                entry.read_to_string(&mut xml).unwrap();
                for axis in ["x", "y", "z"] {
                    xml = xml.replace(&format!("{axis}=\"2\""), &format!("{axis}=\"100000\""));
                }
                package
                    .start_file("3D/3dmodel.model", zip::write::SimpleFileOptions::default())
                    .unwrap();
                package.write_all(xml.as_bytes()).unwrap();
            } else {
                package.raw_copy_file(entry).unwrap();
            }
        }
        let output = source_to_production(
            "small-scale.3mf",
            &package.finish().unwrap().into_inner(),
            &ConversionLimits::default(),
            &ThreeMfLimits::default(),
        )
        .unwrap();
        assert_eq!(output.dimensions_mm, [10.; 3]);
    }

    #[test]
    fn three_mf_normalization_is_lazy_preserves_surrounding_xml_and_checks_deadline() {
        let prefix = "<?xml version=\"1.0\"?>\n<!--preserve me--> <model>\n<resources/> <build>";
        let suffix = "</build>  <!--also preserve me-->\n</model>";
        let xml = format!(
            "{prefix}<item objectid=\"1\" transform=\"-1 0 0 0 1 0 0 0 1 0 0 0\"/>{suffix}"
        );
        let (normalized, _) =
            three_mf_mirrors::normalize(&xml, &ConversionLimits::default(), Instant::now())
                .unwrap();
        let normalized = normalized.unwrap();
        assert!(normalized.starts_with(prefix));
        assert!(normalized.ends_with(suffix));
        assert!(
            three_mf_mirrors::normalize(&normalized, &ConversionLimits::default(), Instant::now())
                .unwrap()
                .0
                .is_none()
        );
        let expired = Instant::now() - Duration::from_secs(2);
        for xml in [xml.as_str(), "<model/>"] {
            assert!(matches!(
                three_mf_mirrors::normalize(
                    xml,
                    &ConversionLimits {
                        max_parse_seconds: 1,
                        ..Default::default()
                    },
                    expired
                ),
                Err(PreviewError::ParseTimeout)
            ));
        }
    }

    #[test]
    fn three_mf_entry_cap_accepts_512_rejects_513() {
        let base = mirror_package("1 0 0 0 1 0 0 0 1 0 0 0", None, 0);
        let count = zip::ZipArchive::new(Cursor::new(base)).unwrap().len();
        for total in [512, 513] {
            let input = mirror_package("1 0 0 0 1 0 0 0 1 0 0 0", None, total - count);
            let result = validate_3mf_container(&input, &ThreeMfLimits::default());
            if total == 512 {
                result.unwrap();
            } else {
                assert_eq!(result.unwrap_err(), PreviewError::TooManyContainerEntries);
            }
        }
    }

    #[test]
    fn three_mf_container_byte_boundaries() {
        let mib = 1024 * 1024;
        let limits = ThreeMfLimits::default();
        for (sizes, accepted) in [
            (vec![96 * mib], true),
            (vec![96 * mib + 1], false),
            (vec![48 * mib, 48 * mib], true),
            (vec![48 * mib, 48 * mib, 1], false),
        ] {
            let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .compression_level(Some(1));
            for (index, &size) in sizes.iter().enumerate() {
                let name = if index == 0 {
                    "3D/3dmodel.model".to_owned()
                } else {
                    format!("padding_{index}.bin")
                };
                archive.start_file(name, options).unwrap();
                std::io::copy(&mut std::io::repeat(0).take(size), &mut archive).unwrap();
            }
            // Exercise only the container gate; zero-filled entries are not valid geometry.
            let input = archive.finish().unwrap().into_inner();
            let result = validate_3mf_container(&input, &limits);
            assert_eq!(
                result,
                if accepted {
                    Ok(())
                } else {
                    Err(PreviewError::ContainerTooLarge)
                },
                "entry sizes: {sizes:?}"
            );
        }
    }

    #[test]
    fn zip64_small_export_preserves_geometry_and_container_limits() {
        let input = include_bytes!("../tests/fixtures/zip64_model.3mf");
        let ordinary = source_to_production(
            "ordinary.3mf",
            THREEMF_SIMPLE,
            &ConversionLimits::default(),
            &ThreeMfLimits::default(),
        )
        .unwrap();
        let zip64 = source_to_production(
            "export.3mf",
            input,
            &ConversionLimits::default(),
            &ThreeMfLimits::default(),
        )
        .unwrap();
        assert_eq!(zip64.dimensions_mm, ordinary.dimensions_mm);
        assert_eq!(zip64.stl, ordinary.stl);
        assert_eq!(
            gate_source_format(
                "export.3mf",
                input,
                &ThreeMfLimits {
                    max_entry_bytes: 100,
                    ..ThreeMfLimits::default()
                }
            )
            .unwrap_err(),
            PreviewError::ContainerTooLarge
        );
    }

    #[test]
    fn production_parts_resolve_scoped_ids_units_and_transform() {
        let input = include_bytes!("../tests/fixtures/production_parts.3mf");
        let out = source_to_production(
            "part.3mf",
            input,
            &ConversionLimits::default(),
            &ThreeMfLimits::default(),
        )
        .unwrap();
        assert!((out.dimensions_mm[0] - 200.0).abs() < 0.001);
        assert!((out.dimensions_mm[2] - 200.0).abs() < 0.001);
        let (scene, _) = parse_stl(&out.stl, &ConversionLimits::default(), Instant::now()).unwrap();
        let min = scene
            .positions_mm
            .iter()
            .map(|p| p[0])
            .fold(f32::INFINITY, f32::min);
        assert!((min - 30.0).abs() < 0.001);
        assert_eq!(
            three_mf_part_path(Some("/../outside.model"), "3D/3dmodel.model"),
            Err(PreviewError::UnsafeContainer)
        );
        assert_eq!(
            three_mf_part_path(Some("https://host/part.model"), "3D/3dmodel.model"),
            Err(PreviewError::UnsafeContainer)
        );
    }

    #[test]
    fn bambu_triangle_painting_preserves_geometry_and_rejects_unknown_attributes() {
        for input in [
            THREEMF_SIMPLE,
            include_bytes!("../tests/fixtures/production_parts.3mf").as_slice(),
        ] {
            assert!(matches!(
                geometry_package(input, &ConversionLimits::default(), Instant::now())
                    .unwrap()
                    .0,
                std::borrow::Cow::Borrowed(_)
            ));
            let baseline = source_to_production(
                "part.3mf",
                input,
                &ConversionLimits::default(),
                &ThreeMfLimits::default(),
            )
            .unwrap();
            for attribute in [
                "paint_supports",
                "paint_seam",
                "paint_color",
                "mmu_segmentation",
                "paint_fuzzy_skin",
                "face_property",
                "unknown_geometry",
            ] {
                let mut source = zip::ZipArchive::new(Cursor::new(input)).unwrap();
                let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
                for index in 0..source.len() {
                    let mut entry = source.by_index(index).unwrap();
                    let name = entry.name().unwrap().into_owned();
                    if name.ends_with(".model") {
                        let mut xml = String::new();
                        entry.read_to_string(&mut xml).unwrap();
                        let xml =
                            xml.replace("<triangle ", &format!("<triangle {attribute}=\"01\" "));
                        output
                            .start_file(name, zip::write::SimpleFileOptions::default())
                            .unwrap();
                        output.write_all(xml.as_bytes()).unwrap();
                    } else {
                        output.raw_copy_file(entry).unwrap();
                    }
                }
                let painted = output.finish().unwrap().into_inner();
                let result = source_to_production(
                    "part.3mf",
                    &painted,
                    &ConversionLimits::default(),
                    &ThreeMfLimits::default(),
                );
                if attribute == "unknown_geometry" {
                    assert!(result.is_err());
                } else {
                    let result = result.unwrap();
                    assert_eq!(baseline.stl, result.stl);
                    assert_eq!(baseline.glb, result.glb);
                    assert_eq!(baseline.dimensions_mm, result.dimensions_mm);
                }
            }
        }
    }

    #[test]
    #[ignore = "requires TEST_MODEL_PATH (legacy TEST_3MF_PATH also accepted)"]
    fn local_real_model() {
        let path = std::env::var("TEST_MODEL_PATH")
            .or_else(|_| std::env::var("TEST_3MF_PATH"))
            .expect("TEST_MODEL_PATH");
        let input = std::fs::read(&path).unwrap();
        let out = source_to_production(
            &path,
            &input,
            &ConversionLimits::default(),
            &ThreeMfLimits::default(),
        )
        .unwrap();
        assert!(out.dimensions_mm.iter().all(|v| v.is_finite() && *v > 0.0));
        assert_eq!(&out.glb[..4], b"glTF");
    }

    fn signed_volume(scene: &MeshScene) -> f64 {
        scene
            .indices
            .chunks_exact(3)
            .map(|triangle| {
                let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                    .map(|index| scene.positions_mm[index as usize].map(f64::from));
                (a[0] * (b[1] * c[2] - b[2] * c[1])
                    + a[1] * (b[2] * c[0] - b[0] * c[2])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]))
                    / 6.0
            })
            .sum()
    }

    #[test]
    fn canonical_stl_writes_unit_normals_with_preserved_winding() {
        let input = b"solid test\nfacet normal 0 0 0\nouter loop\nvertex 0 0 54\nvertex 0 161 57\nvertex 0 161 54\nendloop\nendfacet\nfacet normal 0 0 0\nouter loop\nvertex 0 0 54\nvertex 0 161 54\nvertex 1 0 54\nendloop\nendfacet\nendsolid test";
        let output = source_to_stl(
            "offset.stl",
            input,
            &ConversionLimits::default(),
            &ThreeMfLimits::default(),
        )
        .unwrap();
        let normal: Vec<f32> = output[84..96]
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(normal, [-1.0, 0.0, 0.0]);
        assert_eq!(triangle_normal([[0.0; 3]; 3]), [0.0; 3]);
        assert_eq!(
            triangle_normal([[0.0; 3], [f32::MAX, 0.0, 0.0], [0.0, f32::MAX, 0.0]]),
            [0.0, 0.0, 1.0]
        );
    }

    #[test]
    fn canonical_stl_preserves_geometry_without_project_payloads() {
        let limits = ConversionLimits::default();
        for (filename, input) in [
            ("cube.stl", CUBE_BINARY),
            ("cube.stl", CUBE_ASCII),
            ("part.step", STEP_BLOCK),
            ("part.STP", STEP_CYLINDER),
            ("part.3mf", THREEMF_SIMPLE),
            (
                "transformed.3mf",
                include_bytes!("../tests/fixtures/centimeter_transformed.3mf").as_slice(),
            ),
        ] {
            let format = gate_source_format(filename, input, &ThreeMfLimits::default())
                .unwrap()
                .source_format;
            let (original, format) =
                scene_from_source(format, input, &limits, Instant::now()).unwrap();
            let production =
                source_to_production(filename, input, &limits, &ThreeMfLimits::default()).unwrap();
            let preview = gltf::Gltf::from_slice(&production.glb).unwrap();
            assert!(preview.blob.is_some());
            assert_eq!(preview.buffers().count(), 1);
            assert!(
                preview
                    .buffers()
                    .all(|buffer| matches!(buffer.source(), gltf::buffer::Source::Bin))
            );
            assert!(preview.images().next().is_none());
            assert!(preview.materials().next().is_none());
            let (_, rotation, scale) = preview.nodes().next().unwrap().transform().decomposed();
            assert_eq!(scale, [0.001; 3]);
            assert!((rotation[0] + std::f32::consts::FRAC_1_SQRT_2).abs() < 0.00001);
            let primitive = preview
                .meshes()
                .next()
                .unwrap()
                .primitives()
                .next()
                .unwrap();
            let reader = primitive.reader(|_| preview.blob.as_deref());
            assert_eq!(
                reader.read_positions().unwrap().count(),
                original.positions_mm.len()
            );
            assert_eq!(
                reader
                    .read_indices()
                    .unwrap()
                    .into_u32()
                    .collect::<Vec<_>>(),
                original.indices
            );
            let stl = production.stl;
            let (canonical, _) = parse_stl(&stl, &limits, Instant::now()).unwrap();
            let before = analyze_scene(&original, format, &limits).unwrap();
            assert_eq!(
                production.dimensions_mm,
                before.dimensions_mm.map(f64::from),
                "{filename}"
            );
            let after = analyze_scene(&canonical, SourceFormat::StlBinary, &limits).unwrap();
            assert_eq!(before.bounds_min_mm, after.bounds_min_mm, "{filename}");
            assert_eq!(before.bounds_max_mm, after.bounds_max_mm, "{filename}");
            assert_eq!(before.triangle_count, after.triangle_count, "{filename}");
            assert!(
                (signed_volume(&original) - signed_volume(&canonical)).abs() < 0.0001,
                "{filename}"
            );
            assert_eq!(stl.len(), 84 + before.triangle_count * 50);
            assert!(stl[..80].iter().all(|byte| *byte == 0));
            if filename == "transformed.3mf" {
                assert_eq!(after.bounds_min_mm[0], 30.0);
                assert_eq!(after.bounds_min_mm[2], 50.0);
                assert_eq!(after.dimensions_mm[0], 200.0);
            }
        }
    }

    #[test]
    fn step_tessellation_preserves_millimetres_winding_and_limits() {
        let limits = ConversionLimits::default();
        let mesh = scene_from_step(STEP_BLOCK, &limits, Instant::now()).unwrap();
        let analysis = analyze_scene(&mesh, SourceFormat::Step, &limits).unwrap();
        assert_eq!(analysis.dimensions_mm, [2.0; 3]);
        assert!((signed_volume(&mesh) - 8.0).abs() < 0.0001);
        for (limits, expected) in [
            (
                ConversionLimits {
                    max_triangles: 1,
                    ..Default::default()
                },
                PreviewError::TooManyTriangles,
            ),
            (
                ConversionLimits {
                    max_vertices: 2,
                    ..Default::default()
                },
                PreviewError::TooManyVertices,
            ),
        ] {
            assert_eq!(
                source_to_stl("part.step", STEP_BLOCK, &limits, &ThreeMfLimits::default())
                    .unwrap_err(),
                expected
            );
        }
    }

    #[test]
    fn step_rejects_multiple_solids_in_one_representation() {
        let source = std::str::from_utf8(STEP_BLOCK)
            .unwrap()
            .replace("(#11,#15)", "(#11,#15,#99990)")
            .replace(
                "#15 = MANIFOLD_SOLID_BREP('',#16);",
                "#15 = MANIFOLD_SOLID_BREP('',#16);\n#99990 = MANIFOLD_SOLID_BREP('',#16);",
            );
        assert_eq!(
            source_to_stl(
                "parts.step",
                source.as_bytes(),
                &ConversionLimits::default(),
                &ThreeMfLimits::default()
            )
            .unwrap_err(),
            PreviewError::UnsupportedFormat
        );
    }

    #[test]
    fn canonical_stl_rejects_unsupported_step_contexts_and_output_overflow() {
        let source = std::str::from_utf8(STEP_BLOCK).unwrap();
        for input in [
            source.replace("SI_UNIT(.MILLI.,.METRE.)", "SI_UNIT($,.METRE.)"),
            source.replace("ENDSEC;", "#99999=MAPPED_ITEM(#1,#2);ENDSEC;"),
        ] {
            assert_eq!(
                source_to_stl(
                    "part.step",
                    input.as_bytes(),
                    &ConversionLimits::default(),
                    &ThreeMfLimits::default()
                )
                .unwrap_err(),
                PreviewError::UnsupportedFormat
            );
        }
        assert_eq!(
            three_mf_unit_scale_mm("unknown").unwrap_err(),
            PreviewError::UnsupportedFormat
        );
        let limits = ConversionLimits {
            max_output_glb_bytes: 100,
            ..Default::default()
        };
        assert_eq!(
            source_to_stl("cube.stl", CUBE_BINARY, &limits, &ThreeMfLimits::default()).unwrap_err(),
            PreviewError::OutputTooLarge
        );
    }

    #[test]
    fn exact_indexing_preserves_every_triangle_and_accepts_shared_vertices_at_limit() {
        let limits = ConversionLimits {
            max_vertices: 8,
            ..Default::default()
        };
        let (scene, _) = parse_stl(CUBE_BINARY, &limits, Instant::now()).unwrap();
        assert_eq!(scene.positions_mm.len(), 8);
        assert_eq!(scene.indices.len(), 36);
        for (triangle, indices) in CUBE_BINARY[84..]
            .chunks_exact(50)
            .zip(scene.indices.chunks_exact(3))
        {
            for (corner, &index) in indices.iter().enumerate() {
                let expected = [0, 4, 8].map(|offset| {
                    read_f32_le(triangle, 12 + corner * 12 + offset)
                        .unwrap()
                        .to_bits()
                });
                assert_eq!(
                    scene.positions_mm[index as usize].map(f32::to_bits),
                    expected
                );
            }
        }
        // This legacy fixture has inconsistent face winding; preserve its signed sum exactly.
        assert!((signed_volume(&scene) + 8000.0 / 3.0).abs() < 0.0001);
        let binary =
            source_to_production("cube.stl", CUBE_BINARY, &limits, &ThreeMfLimits::default())
                .unwrap();
        let (roundtrip, _) = parse_stl(&binary.stl, &limits, Instant::now()).unwrap();
        assert_eq!(scene, roundtrip);
        assert_eq!(binary.dimensions_mm, [20.0; 3]);
        let (ascii, _) = parse_stl(CUBE_ASCII, &limits, Instant::now()).unwrap();
        assert_eq!(ascii.positions_mm.len(), 8);
        assert!((signed_volume(&ascii) - signed_volume(&scene)).abs() < 0.0001);
    }

    #[test]
    fn indexed_builder_keeps_exact_bits_and_enforces_unique_vertex_and_triangle_limits() {
        let start = Instant::now();
        let mut builder = MeshSceneBuilder::default();
        let limits = ConversionLimits {
            max_vertices: 3,
            max_triangles: 2,
            ..Default::default()
        };
        let a = [0.0; 3];
        let b = [1.0, 0.0, 0.0];
        let c = [0.0, 1.0, 0.0];
        builder.push_triangle(a, b, c, &limits, start).unwrap();
        builder.push_triangle(c, b, a, &limits, start).unwrap();
        assert_eq!(builder.positions_mm.len(), 3);
        assert_eq!(builder.indices, [0, 1, 2, 2, 1, 0]);
        assert_eq!(
            builder.push_triangle(a, b, c, &limits, start).unwrap_err(),
            PreviewError::TooManyTriangles
        );
        for distinct in [[-0.0, 0.0, 0.0], [f32::from_bits(1), 0.0, 0.0]] {
            let mut builder = MeshSceneBuilder::default();
            builder.push_triangle(a, b, c, &limits, start).unwrap();
            assert_eq!(
                builder
                    .push_triangle(distinct, b, c, &limits, start)
                    .unwrap_err(),
                PreviewError::TooManyVertices
            );
        }
        // Stop at the second complete triangle, before reading any later malformed coordinates.
        let ascii = "solid test\nfacet normal 0 0 1\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nvertex 0 1 0\nvertex 1 0 0\nvertex 0 0 0\nvertex invalid\nendsolid";
        let limits = ConversionLimits {
            max_triangles: 1,
            ..limits
        };
        assert_eq!(
            parse_ascii_stl(ascii, &limits, start).unwrap_err(),
            PreviewError::TooManyTriangles
        );
    }

    #[test]
    fn stl_binary_cube_writes_glb_magic_and_sane_dimensions() {
        let output = stl_to_preview_glb(CUBE_BINARY, &ConversionLimits::default()).unwrap();
        assert_eq!(&output.glb[..4], b"glTF");
        assert_eq!(output.analysis.source_format, SourceFormat::StlBinary);
        assert_eq!(output.analysis.triangle_count, 12);
        assert_eq!(output.analysis.vertex_count, 8);
        assert_eq!(output.analysis.dimensions_mm, [20.0, 20.0, 20.0]);
        assert_eq!(output.analysis.output_glb_bytes, output.glb.len());
    }

    #[test]
    fn stl_ascii_cube_writes_glb_magic_and_sane_dimensions() {
        let output = stl_to_preview_glb(CUBE_ASCII, &ConversionLimits::default()).unwrap();
        assert_eq!(&output.glb[..4], b"glTF");
        assert_eq!(output.analysis.source_format, SourceFormat::StlAscii);
        assert_eq!(output.analysis.triangle_count, 12);
        assert_eq!(output.analysis.dimensions_mm, [20.0, 20.0, 20.0]);
    }

    #[test]
    fn stl_fixture_extremes_do_not_panic_and_keep_bounds_sane() {
        let tiny = stl_to_preview_glb(TINY_CUBE, &ConversionLimits::default()).unwrap();
        assert_eq!(tiny.analysis.dimensions_mm, [1.0, 1.0, 1.0]);

        let huge = stl_to_preview_glb(HUGE_CUBE, &ConversionLimits::default()).unwrap();
        assert_eq!(huge.analysis.dimensions_mm, [1000.0, 1000.0, 1000.0]);

        let off_center = stl_to_preview_glb(OFF_CENTER_CUBE, &ConversionLimits::default()).unwrap();
        assert_eq!(off_center.analysis.dimensions_mm, [20.0, 20.0, 20.0]);
        assert!(off_center.analysis.bounds_min_mm[0] > 900.0);
    }

    #[test]
    fn stl_invalid_malformed_nan_and_inf_return_typed_errors() {
        assert_eq!(
            stl_to_preview_glb(INVALID, &ConversionLimits::default()).unwrap_err(),
            PreviewError::MalformedInput
        );
        assert_eq!(
            stl_to_preview_glb(MALFORMED, &ConversionLimits::default()).unwrap_err(),
            PreviewError::MalformedInput
        );
        assert_eq!(
            stl_to_preview_glb(NAN_INF, &ConversionLimits::default()).unwrap_err(),
            PreviewError::NonFiniteCoordinate
        );
    }

    #[test]
    fn stl_conversion_respects_size_parse_vertex_triangle_and_output_limits() {
        let tiny_input_limit = ConversionLimits {
            max_input_bytes: 10,
            ..ConversionLimits::default()
        };
        assert_eq!(
            stl_to_preview_glb(CUBE_BINARY, &tiny_input_limit).unwrap_err(),
            PreviewError::InputTooLarge
        );

        let tiny_triangle_limit = ConversionLimits {
            max_triangles: 1,
            ..ConversionLimits::default()
        };
        assert_eq!(
            stl_to_preview_glb(CUBE_BINARY, &tiny_triangle_limit).unwrap_err(),
            PreviewError::TooManyTriangles
        );

        let tiny_vertex_limit = ConversionLimits {
            max_vertices: 3,
            ..ConversionLimits::default()
        };
        assert_eq!(
            stl_to_preview_glb(CUBE_BINARY, &tiny_vertex_limit).unwrap_err(),
            PreviewError::TooManyVertices
        );

        let tiny_output_limit = ConversionLimits {
            max_output_glb_bytes: 128,
            ..ConversionLimits::default()
        };
        assert_eq!(
            stl_to_preview_glb(CUBE_BINARY, &tiny_output_limit).unwrap_err(),
            PreviewError::OutputTooLarge
        );
    }

    #[test]
    fn stl_conversion_rejects_coordinate_limit_and_elapsed_parse_limit() {
        let coordinate_limit = ConversionLimits {
            max_coordinate_abs_mm: 1.0,
            ..ConversionLimits::default()
        };
        assert_eq!(
            stl_to_preview_glb(CUBE_BINARY, &coordinate_limit).unwrap_err(),
            PreviewError::CoordinateOutOfBounds
        );

        let timeout_limit = ConversionLimits {
            max_parse_seconds: 0,
            ..ConversionLimits::default()
        };
        assert_eq!(
            parse_stl(
                CUBE_BINARY,
                &timeout_limit,
                Instant::now() - Duration::from_secs(1)
            )
            .unwrap_err(),
            PreviewError::ParseTimeout
        );
    }

    #[test]
    fn compressed_3mf_relationship_entries_are_scanned_for_external_targets() {
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        archive.start_file("3D/3dmodel.model", options).unwrap();
        archive.write_all(b"unused").unwrap();
        archive.start_file("_rels/.rels", options).unwrap();
        archive.write_all(br#"<Relationships><Relationship TargetMode="External" Target="http://example.com"/></Relationships>"#).unwrap();
        let bytes = archive.finish().unwrap().into_inner();
        assert_eq!(
            validate_3mf_container(&bytes, &ThreeMfLimits::default()),
            Err(PreviewError::ExternalReferenceDisabled)
        );
    }

    #[test]
    fn unsupported_inputs_are_rejected_by_preview_and_production() {
        let limits = ConversionLimits::default();
        let three_mf_limits = ThreeMfLimits::default();
        for (filename, input) in [
            ("single_cube.obj", OBJ_SINGLE_CUBE),
            ("simple.ply", PLY_SIMPLE),
            ("simple.glb", GLB_SIMPLE),
            ("embedded_triangle.gltf", GLTF_EMBEDDED_TRIANGLE),
            ("external.gltf", GLTF_EXTERNAL),
            ("unknown.exe", CUBE_ASCII),
            ("unknown", CUBE_BINARY),
        ] {
            let gate = gate_source_format(filename, input, &three_mf_limits).unwrap();
            assert_eq!(gate.status, FormatSupportStatus::Unsupported, "{filename}");
            assert_eq!(gate.reason_code, "UNSUPPORTED_FORMAT", "{filename}");
            assert_eq!(
                source_to_preview_glb(filename, input, &limits, &three_mf_limits).unwrap_err(),
                PreviewError::UnsupportedFormat,
                "{filename}"
            );
            assert_eq!(
                source_to_production(filename, input, &limits, &three_mf_limits)
                    .err()
                    .unwrap(),
                PreviewError::UnsupportedFormat,
                "{filename}"
            );
        }
    }

    #[test]
    fn preview_conversion_covers_supported_smoke_formats_through_unified_glb_output() {
        let limits = ConversionLimits::default();
        let three_mf_limits = ThreeMfLimits::default();
        for (filename, bytes, expected_format) in [
            ("cube.STL", CUBE_BINARY, SourceFormat::StlBinary),
            ("cube_ascii.stl", CUBE_ASCII, SourceFormat::StlAscii),
            ("step_block.STP", STEP_BLOCK, SourceFormat::Step),
            ("simple_model.3mf", THREEMF_SIMPLE, SourceFormat::ThreeMf),
            ("step_block.step", STEP_BLOCK, SourceFormat::Step),
        ] {
            let output = match source_to_preview_glb(filename, bytes, &limits, &three_mf_limits) {
                Ok(output) => output,
                Err(err) => panic!("{filename}: {err:?}"),
            };
            assert_eq!(output.analysis.source_format, expected_format, "{filename}");
            assert_eq!(&output.glb[..4], b"glTF", "{filename}");
            assert_eq!(
                output.analysis.output_glb_bytes,
                output.glb.len(),
                "{filename}"
            );
            assert!(output.analysis.triangle_count > 0, "{filename}");
        }
    }

    #[test]
    fn three_mf_security_gate_rejects_unsafe_containers_without_panic() {
        let limits = ThreeMfLimits::default();
        assert_eq!(
            gate_source_format("zip_path_traversal.3mf", THREEMF_TRAVERSAL, &limits).unwrap_err(),
            PreviewError::UnsafeContainer
        );
        assert_eq!(
            gate_source_format("zip_duplicate_normalized.3mf", THREEMF_DUPLICATE, &limits)
                .unwrap_err(),
            PreviewError::DuplicateContainerPath
        );
        assert_eq!(
            gate_source_format(
                "zip_external_relationship.3mf",
                THREEMF_EXTERNAL_REL,
                &limits
            )
            .unwrap_err(),
            PreviewError::ExternalReferenceDisabled
        );
        assert_eq!(
            gate_source_format(
                "zip_bomb_limit.3mf",
                THREEMF_BOMB,
                &ThreeMfLimits {
                    max_entry_count: 10,
                    max_uncompressed_bytes: 512,
                    max_entry_bytes: 512,
                },
            )
            .unwrap_err(),
            PreviewError::ContainerTooLarge
        );
        assert_eq!(
            gate_source_format(
                "simple_model.3mf",
                THREEMF_SIMPLE,
                &ThreeMfLimits {
                    max_entry_count: 1,
                    ..ThreeMfLimits::default()
                },
            )
            .unwrap_err(),
            PreviewError::TooManyContainerEntries
        );
    }

    #[test]
    fn step_and_stp_are_safe_manual_review_gates_not_instant_preview_or_quote() {
        for (filename, bytes) in [
            ("step_block.step", STEP_BLOCK),
            ("step_cylinder.step", STEP_CYLINDER),
            ("step_unsupported_nurbs.step", STEP_UNSUPPORTED_NURBS),
        ] {
            let gate = gate_source_format(filename, bytes, &ThreeMfLimits::default()).unwrap();
            assert_eq!(gate.source_format, SourceFormat::Step, "{filename}");
            assert_eq!(gate.status, FormatSupportStatus::ManualReviewRequired);
            assert_eq!(gate.reason_code, "STEP_MANUAL_REVIEW_REQUIRED");
        }
        let stp = gate_source_format("block.stp", STEP_BLOCK, &ThreeMfLimits::default()).unwrap();
        assert_eq!(stp.status, FormatSupportStatus::ManualReviewRequired);
        assert_eq!(
            gate_source_format(
                "external.step",
                b"ISO-10303-21; HEADER; ENDSEC; DATA; #1=EXTERNAL_FILE('x'); ENDSEC; END-ISO-10303-21;",
                &ThreeMfLimits::default(),
            )
            .unwrap_err(),
            PreviewError::ExternalReferenceDisabled
        );
    }
}
