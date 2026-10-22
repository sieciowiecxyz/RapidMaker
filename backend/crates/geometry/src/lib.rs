use std::{
    collections::HashMap,
    io::{Cursor, Read},
    time::{Duration, Instant},
};

use flate2::read::DeflateDecoder;
use lib3mf::{Model as ThreeMfModel, ParserConfig, SpecConformance};
use ply_rs_bw::{
    parser::Parser as PlyParser,
    ply::{DefaultElement, Property},
};
use truck_meshalgo::prelude::*;
use truck_polymesh::{Point3, PolygonMesh, obj};
use truck_stepio::r#in::Table;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFormat {
    StlBinary,
    StlAscii,
    Obj,
    Ply,
    Glb,
    Gltf,
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
            max_entry_count: 256,
            max_uncompressed_bytes: 50 * 1024 * 1024,
            max_entry_bytes: 25 * 1024 * 1024,
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
            max_triangles: 1_000_000,
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
    let (scene, source_format) = scene_from_source(gate.source_format, input, limits, started)?;
    build_preview_output(scene, source_format, limits)
}

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
    let glb = write_glb(&scene, &analysis)?;
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
        SourceFormat::Obj => {
            scene_from_obj(input, limits, started).map(|scene| (scene, source_format))
        }
        SourceFormat::Ply => {
            scene_from_ply(input, limits, started).map(|scene| (scene, source_format))
        }
        SourceFormat::Glb | SourceFormat::Gltf => {
            scene_from_gltf(input, limits, started).map(|scene| (scene, source_format))
        }
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
    if lower.ends_with(".obj") {
        return gate_obj(input);
    }
    if lower.ends_with(".ply") {
        return gate_ply(input);
    }
    if lower.ends_with(".glb") {
        validate_glb(input)?;
        return Ok(FormatGateResult {
            source_format: SourceFormat::Glb,
            status: FormatSupportStatus::PreviewSupported,
            reason_code: "GLB_PREVIEW_SUPPORTED",
        });
    }
    if lower.ends_with(".gltf") {
        return gate_gltf(input);
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

fn gate_obj(input: &[u8]) -> Result<FormatGateResult, PreviewError> {
    let text = std::str::from_utf8(input).map_err(|_| PreviewError::MalformedInput)?;
    if text.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("mtllib ") || trimmed.starts_with("usemtl ")
    }) {
        return Ok(FormatGateResult {
            source_format: SourceFormat::Obj,
            status: FormatSupportStatus::ManualReviewRequired,
            reason_code: "OBJ_MATERIALS_DISABLED",
        });
    }
    let has_vertex = text.lines().any(|line| line.trim_start().starts_with("v "));
    let has_face = text.lines().any(|line| line.trim_start().starts_with("f "));
    if has_vertex && has_face {
        Ok(FormatGateResult {
            source_format: SourceFormat::Obj,
            status: FormatSupportStatus::PreviewSupported,
            reason_code: "OBJ_SINGLE_FILE_PREVIEW_SUPPORTED",
        })
    } else {
        Err(PreviewError::MalformedInput)
    }
}

fn gate_ply(input: &[u8]) -> Result<FormatGateResult, PreviewError> {
    let header_end = input
        .windows("end_header".len())
        .position(|window| window == b"end_header")
        .ok_or(PreviewError::MalformedInput)?;
    let header =
        std::str::from_utf8(&input[..header_end]).map_err(|_| PreviewError::MalformedInput)?;
    let supported_format =
        header.contains("format ascii 1.0") || header.contains("format binary_little_endian 1.0");
    if header.starts_with("ply")
        && supported_format
        && header.contains("element vertex ")
        && header.contains("element face ")
    {
        Ok(FormatGateResult {
            source_format: SourceFormat::Ply,
            status: FormatSupportStatus::PreviewSupported,
            reason_code: "PLY_SIMPLE_MESH_PREVIEW_SUPPORTED",
        })
    } else {
        Err(PreviewError::MalformedInput)
    }
}

fn validate_glb(input: &[u8]) -> Result<(), PreviewError> {
    if input.len() < 20 || !input.starts_with(b"glTF") {
        return Err(PreviewError::MalformedInput);
    }
    let version = u32::from_le_bytes(input[4..8].try_into().expect("slice length"));
    let declared_len = u32::from_le_bytes(input[8..12].try_into().expect("slice length")) as usize;
    if version != 2 || declared_len != input.len() {
        return Err(PreviewError::MalformedInput);
    }
    let glb = gltf::binary::Glb::from_slice(input).map_err(|_| PreviewError::MalformedInput)?;
    let value: serde_json::Value =
        serde_json::from_slice(&glb.json).map_err(|_| PreviewError::MalformedInput)?;
    if gltf_has_external_uri(&value) {
        return Err(PreviewError::ExternalReferenceDisabled);
    }
    Ok(())
}

fn gate_gltf(input: &[u8]) -> Result<FormatGateResult, PreviewError> {
    let text = std::str::from_utf8(input).map_err(|_| PreviewError::MalformedInput)?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| PreviewError::MalformedInput)?;
    if gltf_has_external_uri(&value) {
        return Err(PreviewError::ExternalReferenceDisabled);
    }
    if value
        .get("asset")
        .and_then(|asset| asset.get("version"))
        .is_some()
    {
        Ok(FormatGateResult {
            source_format: SourceFormat::Gltf,
            status: FormatSupportStatus::PreviewSupported,
            reason_code: "GLTF_EMBEDDED_PREVIEW_SUPPORTED",
        })
    } else {
        Err(PreviewError::MalformedInput)
    }
}

fn gltf_has_external_uri(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => map.iter().any(|(key, value)| {
            if key == "uri" {
                value.as_str().is_none_or(|uri| !uri.starts_with("data:"))
            } else {
                gltf_has_external_uri(value)
            }
        }),
        serde_json::Value::Array(values) => values.iter().any(gltf_has_external_uri),
        _ => false,
    }
}

fn validate_3mf_container(input: &[u8], limits: &ThreeMfLimits) -> Result<(), PreviewError> {
    if input.len() < 22 {
        return Err(PreviewError::MalformedInput);
    }
    let eocd = find_zip_eocd(input).ok_or(PreviewError::MalformedInput)?;
    let entry_count = read_u16_le(input, eocd + 10)? as usize;
    let central_size = read_u32_le(input, eocd + 12)? as usize;
    let central_offset = read_u32_le(input, eocd + 16)? as usize;
    if entry_count > limits.max_entry_count {
        return Err(PreviewError::TooManyContainerEntries);
    }
    if central_offset
        .checked_add(central_size)
        .is_none_or(|end| end > input.len())
    {
        return Err(PreviewError::MalformedInput);
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut cursor = central_offset;
    let mut total_uncompressed = 0_u64;
    let mut saw_model = false;
    for _ in 0..entry_count {
        if input.get(cursor..cursor + 4) != Some(b"PK\x01\x02") {
            return Err(PreviewError::MalformedInput);
        }
        let method = read_u16_le(input, cursor + 10)?;
        let compressed = read_u32_le(input, cursor + 20)? as u64;
        let uncompressed = read_u32_le(input, cursor + 24)? as u64;
        let name_len = read_u16_le(input, cursor + 28)? as usize;
        let extra_len = read_u16_le(input, cursor + 30)? as usize;
        let comment_len = read_u16_le(input, cursor + 32)? as usize;
        let local_offset = read_u32_le(input, cursor + 42)? as usize;
        let name_start = cursor + 46;
        let name_end = name_start + name_len;
        let name = std::str::from_utf8(
            input
                .get(name_start..name_end)
                .ok_or(PreviewError::MalformedInput)?,
        )
        .map_err(|_| PreviewError::UnsafeContainer)?;
        let normalized = normalize_container_path(name)?;
        if !seen.insert(normalized.clone()) {
            return Err(PreviewError::DuplicateContainerPath);
        }
        total_uncompressed = total_uncompressed
            .checked_add(uncompressed)
            .ok_or(PreviewError::ContainerTooLarge)?;
        if uncompressed > limits.max_entry_bytes
            || total_uncompressed > limits.max_uncompressed_bytes
        {
            return Err(PreviewError::ContainerTooLarge);
        }
        if normalized == "3D/3dmodel.model" {
            saw_model = true;
        }
        if normalized.ends_with(".rels") {
            inspect_relationship_entry(input, local_offset, method, compressed as usize)?;
        }
        cursor = name_end + extra_len + comment_len;
    }
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

fn inspect_relationship_entry(
    input: &[u8],
    local_offset: usize,
    method: u16,
    compressed_len: usize,
) -> Result<(), PreviewError> {
    if input.get(local_offset..local_offset + 4) != Some(b"PK\x03\x04") {
        return Err(PreviewError::MalformedInput);
    }
    let name_len = read_u16_le(input, local_offset + 26)? as usize;
    let extra_len = read_u16_le(input, local_offset + 28)? as usize;
    let data_start = local_offset + 30 + name_len + extra_len;
    let data = input
        .get(data_start..data_start + compressed_len)
        .ok_or(PreviewError::MalformedInput)?;
    let text = match method {
        0 => std::str::from_utf8(data)
            .map_err(|_| PreviewError::MalformedInput)?
            .to_owned(),
        8 => {
            let mut text = String::new();
            DeflateDecoder::new(data)
                .read_to_string(&mut text)
                .map_err(|_| PreviewError::MalformedInput)?;
            text
        }
        _ => return Err(PreviewError::ExternalReferenceDisabled),
    };
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

fn find_zip_eocd(input: &[u8]) -> Option<usize> {
    input.windows(4).rposition(|window| window == b"PK\x05\x06")
}

fn read_u16_le(input: &[u8], offset: usize) -> Result<u16, PreviewError> {
    let bytes = input
        .get(offset..offset + 2)
        .ok_or(PreviewError::MalformedInput)?;
    Ok(u16::from_le_bytes(bytes.try_into().expect("slice length")))
}

fn read_u32_le(input: &[u8], offset: usize) -> Result<u32, PreviewError> {
    let bytes = input
        .get(offset..offset + 4)
        .ok_or(PreviewError::MalformedInput)?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("slice length")))
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
    let vertex_count = triangle_count
        .checked_mul(3)
        .ok_or(PreviewError::TooManyVertices)?;
    if vertex_count > limits.max_vertices {
        return Err(PreviewError::TooManyVertices);
    }

    let mut positions = Vec::with_capacity(vertex_count);
    let mut indices = Vec::with_capacity(vertex_count);
    let mut cursor = 84;
    for triangle_index in 0..triangle_count {
        check_timeout(started, limits)?;
        cursor += 12;
        for _ in 0..3 {
            positions.push([
                read_f32_le(input, cursor)?,
                read_f32_le(input, cursor + 4)?,
                read_f32_le(input, cursor + 8)?,
            ]);
            indices.push(indices.len() as u32);
            cursor += 12;
        }
        cursor += 2;
        if triangle_index == triangle_count.saturating_sub(1) && cursor != input.len() {
            return Err(PreviewError::MalformedInput);
        }
    }
    Ok(Some(MeshScene {
        positions_mm: positions,
        indices,
    }))
}

fn parse_ascii_stl(
    text: &str,
    limits: &ConversionLimits,
    started: Instant,
) -> Result<MeshScene, PreviewError> {
    let mut positions = Vec::new();
    let mut indices = Vec::new();
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
                positions.push([x, y, z]);
                indices.push(indices.len() as u32);
            }
            _ => {}
        }
    }
    if !saw_facet || positions.is_empty() || positions.len() % 3 != 0 {
        return Err(PreviewError::MalformedInput);
    }
    let triangle_count = positions.len() / 3;
    if triangle_count > limits.max_triangles {
        return Err(PreviewError::TooManyTriangles);
    }
    if positions.len() > limits.max_vertices {
        return Err(PreviewError::TooManyVertices);
    }
    Ok(MeshScene {
        positions_mm: positions,
        indices,
    })
}

fn scene_from_obj(
    input: &[u8],
    limits: &ConversionLimits,
    started: Instant,
) -> Result<MeshScene, PreviewError> {
    let mesh = obj::read(Cursor::new(input)).map_err(|_| PreviewError::MalformedInput)?;
    let mut builder = MeshSceneBuilder::default();
    emit_standard_polygon_mesh(&mesh, limits, started, &mut builder)?;
    builder.finish()
}

fn scene_from_ply(
    input: &[u8],
    limits: &ConversionLimits,
    started: Instant,
) -> Result<MeshScene, PreviewError> {
    let parser = PlyParser::<DefaultElement>::new();
    let mut reader = Cursor::new(input);
    let ply = parser
        .read_ply(&mut reader)
        .map_err(|_| PreviewError::MalformedInput)?;

    let vertices = ply
        .payload
        .get("vertex")
        .ok_or(PreviewError::MalformedInput)?;
    let mut positions = Vec::with_capacity(vertices.len());
    for vertex in vertices {
        check_timeout(started, limits)?;
        positions.push([
            ply_scalar_as_f32(vertex.get("x"))?,
            ply_scalar_as_f32(vertex.get("y"))?,
            ply_scalar_as_f32(vertex.get("z"))?,
        ]);
    }

    let faces = ply
        .payload
        .get("face")
        .ok_or(PreviewError::MalformedInput)?;
    let mut builder = MeshSceneBuilder::default();
    for face in faces {
        check_timeout(started, limits)?;
        let indices = ply_face_indices(face)?;
        if indices.len() < 3 {
            continue;
        }
        if indices.iter().any(|index| *index >= positions.len()) {
            return Err(PreviewError::MalformedInput);
        }
        let first = positions[indices[0]];
        for i in 1..indices.len() - 1 {
            let b = positions[indices[i]];
            let c = positions[indices[i + 1]];
            builder.push_triangle(first, b, c, limits, started)?;
        }
    }
    builder.finish()
}

fn scene_from_gltf(
    input: &[u8],
    limits: &ConversionLimits,
    started: Instant,
) -> Result<MeshScene, PreviewError> {
    let (document, buffers, _) =
        gltf::import_slice(input).map_err(|_| PreviewError::MalformedInput)?;
    let mut builder = MeshSceneBuilder::default();
    if let Some(scene) = document.default_scene() {
        emit_gltf_scene(
            &scene,
            &buffers,
            &mut builder,
            limits,
            started,
            mat4_identity(),
        )?;
    } else {
        for scene in document.scenes() {
            emit_gltf_scene(
                &scene,
                &buffers,
                &mut builder,
                limits,
                started,
                mat4_identity(),
            )?;
        }
    }
    builder.finish()
}

fn scene_from_3mf(
    input: &[u8],
    limits: &ConversionLimits,
    started: Instant,
) -> Result<MeshScene, PreviewError> {
    let config =
        ParserConfig::with_all_extensions().with_spec_conformance(SpecConformance::Lenient);
    let model = ThreeMfModel::from_reader_with_config(Cursor::new(input), config)
        .map_err(|_| PreviewError::MalformedInput)?;
    let scale = three_mf_unit_scale_mm(model.unit.as_str());
    let mut object_index_by_id = HashMap::new();
    for (index, object) in model.resources.objects.iter().enumerate() {
        object_index_by_id.insert(object.id, index);
    }

    let mut walker = ThreeMfObjectWalker {
        model: &model,
        object_index_by_id: &object_index_by_id,
        stack: Vec::new(),
        builder: MeshSceneBuilder::default(),
        limits,
        started,
    };
    let root_transform = mat4_scale(scale);

    if model.build.items.is_empty() {
        for object in &model.resources.objects {
            walker.emit_object(object.id, root_transform)?;
        }
    } else {
        for item in &model.build.items {
            let transform = mat4_mul(
                root_transform,
                item.transform
                    .map(mat4_from_3mf_transform)
                    .unwrap_or_else(mat4_identity),
            );
            walker.emit_object(item.objectid, transform)?;
        }
    }

    walker.finish()
}

fn scene_from_step(
    input: &[u8],
    limits: &ConversionLimits,
    started: Instant,
) -> Result<MeshScene, PreviewError> {
    let text = std::str::from_utf8(input).map_err(|_| PreviewError::MalformedInput)?;
    let table = Table::from_step(text).ok_or(PreviewError::MalformedInput)?;
    let mut builder = MeshSceneBuilder::default();
    for step_shell in table.shell.values() {
        check_timeout(started, limits)?;
        let cshell = table
            .to_compressed_shell(step_shell)
            .map_err(|_| PreviewError::MalformedInput)?;
        let polygon = cshell.triangulation(0.01).to_polygon();
        emit_standard_polygon_mesh(&polygon, limits, started, &mut builder)?;
    }
    builder.finish()
}

fn emit_standard_polygon_mesh(
    mesh: &PolygonMesh,
    limits: &ConversionLimits,
    started: Instant,
    builder: &mut MeshSceneBuilder,
) -> Result<(), PreviewError> {
    for face in mesh.face_iter() {
        check_timeout(started, limits)?;
        let mut points = Vec::with_capacity(face.len());
        for vertex in face {
            let attr = mesh
                .attributes()
                .get(*vertex)
                .ok_or(PreviewError::MalformedInput)?;
            points.push(point3_to_mm(attr.position));
        }
        builder.push_polygon(&points, limits, started)?;
    }
    Ok(())
}

fn emit_gltf_scene(
    scene: &gltf::Scene<'_>,
    buffers: &[gltf::buffer::Data],
    builder: &mut MeshSceneBuilder,
    limits: &ConversionLimits,
    started: Instant,
    transform: [[f32; 4]; 4],
) -> Result<(), PreviewError> {
    for node in scene.nodes() {
        emit_gltf_node(&node, buffers, builder, limits, started, transform)?;
    }
    Ok(())
}

fn emit_gltf_node(
    node: &gltf::Node<'_>,
    buffers: &[gltf::buffer::Data],
    builder: &mut MeshSceneBuilder,
    limits: &ConversionLimits,
    started: Instant,
    parent_transform: [[f32; 4]; 4],
) -> Result<(), PreviewError> {
    check_timeout(started, limits)?;
    let transform = mat4_mul(parent_transform, node.transform().matrix());
    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            emit_gltf_primitive(&primitive, buffers, builder, limits, started, transform)?;
        }
    }
    for child in node.children() {
        emit_gltf_node(&child, buffers, builder, limits, started, transform)?;
    }
    Ok(())
}

fn emit_gltf_primitive(
    primitive: &gltf::Primitive<'_>,
    buffers: &[gltf::buffer::Data],
    builder: &mut MeshSceneBuilder,
    limits: &ConversionLimits,
    started: Instant,
    transform: [[f32; 4]; 4],
) -> Result<(), PreviewError> {
    use gltf::mesh::Mode;

    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
    let positions: Vec<[f32; 3]> = reader
        .read_positions()
        .ok_or(PreviewError::MalformedInput)?
        .collect();
    if positions.len() < 3 {
        return Ok(());
    }
    let indices: Vec<u32> = match reader.read_indices() {
        Some(indices) => collect_gltf_indices(indices)?,
        None => (0..u32::try_from(positions.len()).map_err(|_| PreviewError::TooManyVertices)?)
            .collect(),
    };
    match primitive.mode() {
        Mode::Triangles => {
            for triangle in indices.chunks(3) {
                if triangle.len() != 3 {
                    return Err(PreviewError::MalformedInput);
                }
                let a = positions
                    .get(triangle[0] as usize)
                    .ok_or(PreviewError::MalformedInput)?;
                let b = positions
                    .get(triangle[1] as usize)
                    .ok_or(PreviewError::MalformedInput)?;
                let c = positions
                    .get(triangle[2] as usize)
                    .ok_or(PreviewError::MalformedInput)?;
                emit_triangle_points(a, b, c, transform, builder, limits, started)?;
            }
        }
        Mode::TriangleStrip => {
            for index in 0..indices.len().saturating_sub(2) {
                let (a, b, c) = if index % 2 == 0 {
                    (indices[index], indices[index + 1], indices[index + 2])
                } else {
                    (indices[index + 1], indices[index], indices[index + 2])
                };
                let a = positions
                    .get(a as usize)
                    .ok_or(PreviewError::MalformedInput)?;
                let b = positions
                    .get(b as usize)
                    .ok_or(PreviewError::MalformedInput)?;
                let c = positions
                    .get(c as usize)
                    .ok_or(PreviewError::MalformedInput)?;
                emit_triangle_points(a, b, c, transform, builder, limits, started)?;
            }
        }
        Mode::TriangleFan => {
            for index in 1..indices.len().saturating_sub(1) {
                let a = positions
                    .get(indices[0] as usize)
                    .ok_or(PreviewError::MalformedInput)?;
                let b = positions
                    .get(indices[index] as usize)
                    .ok_or(PreviewError::MalformedInput)?;
                let c = positions
                    .get(indices[index + 1] as usize)
                    .ok_or(PreviewError::MalformedInput)?;
                emit_triangle_points(a, b, c, transform, builder, limits, started)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn collect_gltf_indices(
    indices: gltf::mesh::util::ReadIndices<'_>,
) -> Result<Vec<u32>, PreviewError> {
    use gltf::mesh::util::ReadIndices;

    let mut out = Vec::new();
    match indices {
        ReadIndices::U8(iter) => {
            for value in iter {
                out.push(u32::from(value));
            }
        }
        ReadIndices::U16(iter) => {
            for value in iter {
                out.push(u32::from(value));
            }
        }
        ReadIndices::U32(iter) => {
            for value in iter {
                out.push(value);
            }
        }
    }
    Ok(out)
}

struct ThreeMfObjectWalker<'a> {
    model: &'a ThreeMfModel,
    object_index_by_id: &'a HashMap<usize, usize>,
    stack: Vec<usize>,
    builder: MeshSceneBuilder,
    limits: &'a ConversionLimits,
    started: Instant,
}

impl<'a> ThreeMfObjectWalker<'a> {
    fn emit_object(
        &mut self,
        object_id: usize,
        transform: [[f32; 4]; 4],
    ) -> Result<(), PreviewError> {
        if self.stack.contains(&object_id) {
            return Err(PreviewError::MalformedInput);
        }
        let Some(&object_index) = self.object_index_by_id.get(&object_id) else {
            return Err(PreviewError::MalformedInput);
        };
        self.stack.push(object_id);
        let object = &self.model.resources.objects[object_index];
        if let Some(mesh) = &object.mesh {
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
            self.emit_object(component.objectid, mat4_mul(transform, component_transform))?;
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

fn ply_scalar_as_f32(value: Option<&Property>) -> Result<f32, PreviewError> {
    match value.ok_or(PreviewError::MalformedInput)? {
        Property::Float(v) => Ok(*v),
        Property::Double(v) => Ok(*v as f32),
        Property::Int(v) => Ok(*v as f32),
        Property::UInt(v) => Ok(*v as f32),
        Property::Short(v) => Ok(*v as f32),
        Property::UShort(v) => Ok(*v as f32),
        Property::Char(v) => Ok(*v as f32),
        Property::UChar(v) => Ok(*v as f32),
        _ => Err(PreviewError::MalformedInput),
    }
}

fn ply_face_indices(element: &DefaultElement) -> Result<Vec<usize>, PreviewError> {
    let property = element
        .get("vertex_indices")
        .or_else(|| element.get("vertex_index"))
        .ok_or(PreviewError::MalformedInput)?;
    let indices = match property {
        Property::ListChar(values) => values
            .iter()
            .map(|value| *value as isize)
            .collect::<Vec<_>>(),
        Property::ListUChar(values) => values
            .iter()
            .map(|value| *value as isize)
            .collect::<Vec<_>>(),
        Property::ListShort(values) => values
            .iter()
            .map(|value| *value as isize)
            .collect::<Vec<_>>(),
        Property::ListUShort(values) => values
            .iter()
            .map(|value| *value as isize)
            .collect::<Vec<_>>(),
        Property::ListInt(values) => values
            .iter()
            .map(|value| *value as isize)
            .collect::<Vec<_>>(),
        Property::ListUInt(values) => values
            .iter()
            .map(|value| *value as isize)
            .collect::<Vec<_>>(),
        _ => return Err(PreviewError::MalformedInput),
    };
    let mut out = Vec::with_capacity(indices.len());
    for value in indices {
        if value < 0 {
            return Err(PreviewError::MalformedInput);
        }
        out.push(value as usize);
    }
    Ok(out)
}

fn three_mf_unit_scale_mm(unit: &str) -> f32 {
    match unit.to_ascii_lowercase().as_str() {
        "micron" | "micrometer" => 0.001,
        "millimeter" => 1.0,
        "centimeter" => 10.0,
        "inch" => 25.4,
        "foot" => 304.8,
        "meter" => 1000.0,
        _ => 1.0,
    }
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

fn point3_to_mm(point: Point3) -> [f32; 3] {
    [point.x as f32, point.y as f32, point.z as f32]
}

#[derive(Default)]
struct MeshSceneBuilder {
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
        if self.positions_mm.len().saturating_add(3) > limits.max_vertices {
            return Err(PreviewError::TooManyVertices);
        }
        let base =
            u32::try_from(self.positions_mm.len()).map_err(|_| PreviewError::TooManyVertices)?;
        self.positions_mm.push(a);
        self.positions_mm.push(b);
        self.positions_mm.push(c);
        self.indices.push(base);
        self.indices.push(base + 1);
        self.indices.push(base + 2);
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

pub fn write_glb(scene: &MeshScene, analysis: &PreviewAnalysis) -> Result<Vec<u8>, PreviewError> {
    let mut bin = Vec::with_capacity(scene.positions_mm.len() * 12 + scene.indices.len() * 4);
    for vertex in &scene.positions_mm {
        for value in vertex {
            bin.extend_from_slice(&value.to_le_bytes());
        }
    }
    let index_offset = bin.len();
    for index in &scene.indices {
        bin.extend_from_slice(&index.to_le_bytes());
    }
    pad_to_4(&mut bin, 0);

    let json = format!(
        concat!(
            "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"rapidfab-3d-web-preview-generator\"}},",
            "\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],\"nodes\":[{{\"mesh\":0}}],",
            "\"meshes\":[{{\"primitives\":[{{\"attributes\":{{\"POSITION\":0}},\"indices\":1}}]}}],",
            "\"buffers\":[{{\"byteLength\":{buffer_len}}}],",
            "\"bufferViews\":[",
            "{{\"buffer\":0,\"byteOffset\":0,\"byteLength\":{positions_len},\"target\":34962}},",
            "{{\"buffer\":0,\"byteOffset\":{index_offset},\"byteLength\":{indices_len},\"target\":34963}}",
            "],",
            "\"accessors\":[",
            "{{\"bufferView\":0,\"componentType\":5126,\"count\":{vertex_count},\"type\":\"VEC3\",",
            "\"min\":[{min_x},{min_y},{min_z}],\"max\":[{max_x},{max_y},{max_z}]}},",
            "{{\"bufferView\":1,\"componentType\":5125,\"count\":{index_count},\"type\":\"SCALAR\"}}",
            "]}}"
        ),
        buffer_len = bin.len(),
        positions_len = scene.positions_mm.len() * 12,
        index_offset = index_offset,
        indices_len = scene.indices.len() * 4,
        vertex_count = scene.positions_mm.len(),
        index_count = scene.indices.len(),
        min_x = gltf_number(analysis.bounds_min_mm[0]),
        min_y = gltf_number(analysis.bounds_min_mm[1]),
        min_z = gltf_number(analysis.bounds_min_mm[2]),
        max_x = gltf_number(analysis.bounds_max_mm[0]),
        max_y = gltf_number(analysis.bounds_max_mm[1]),
        max_z = gltf_number(analysis.bounds_max_mm[2]),
    );
    let mut json_bytes = json.into_bytes();
    pad_to_4(&mut json_bytes, b' ');

    let total_len = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut glb = Vec::with_capacity(total_len);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(total_len as u32).to_le_bytes());
    glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json_bytes);
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend_from_slice(&bin);
    Ok(glb)
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
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::DeflateEncoder};
    use std::io::Write;

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
    const THREEMF_TRAVERSAL: &[u8] = include_bytes!("../tests/fixtures/zip_path_traversal.3mf");
    const THREEMF_DUPLICATE: &[u8] =
        include_bytes!("../tests/fixtures/zip_duplicate_normalized.3mf");
    const THREEMF_EXTERNAL_REL: &[u8] =
        include_bytes!("../tests/fixtures/zip_external_relationship.3mf");
    const THREEMF_BOMB: &[u8] = include_bytes!("../tests/fixtures/zip_bomb_limit.3mf");
    const GLTF_EXTERNAL: &[u8] = include_bytes!("../tests/fixtures/gltf_external_ref.gltf");
    const GLTF_EMBEDDED: &[u8] = include_bytes!("../tests/fixtures/gltf_embedded_minimal.gltf");
    const GLTF_EMBEDDED_TRIANGLE: &[u8] =
        include_bytes!("../tests/fixtures/gltf_embedded_triangle.gltf");
    const STEP_BLOCK: &[u8] = include_bytes!("../tests/fixtures/step_block.step");
    const STEP_CYLINDER: &[u8] = include_bytes!("../tests/fixtures/step_cylinder.step");
    const STEP_UNSUPPORTED_NURBS: &[u8] =
        include_bytes!("../tests/fixtures/step_unsupported_nurbs.step");

    #[test]
    fn stl_binary_cube_writes_glb_magic_and_sane_dimensions() {
        let output = stl_to_preview_glb(CUBE_BINARY, &ConversionLimits::default()).unwrap();
        assert_eq!(&output.glb[..4], b"glTF");
        assert_eq!(output.analysis.source_format, SourceFormat::StlBinary);
        assert_eq!(output.analysis.triangle_count, 12);
        assert_eq!(output.analysis.vertex_count, 36);
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
    fn format_gates_cover_obj_ply_glb_gltf_and_3mf_smoke_fixtures() {
        let limits = ThreeMfLimits::default();

        let obj = gate_source_format("single_cube.obj", OBJ_SINGLE_CUBE, &limits).unwrap();
        assert_eq!(obj.source_format, SourceFormat::Obj);
        assert_eq!(obj.status, FormatSupportStatus::ManualReviewRequired);
        assert_eq!(obj.reason_code, "OBJ_MATERIALS_DISABLED");

        let obj_simple = gate_source_format(
            "plain.obj",
            b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n",
            &limits,
        )
        .unwrap();
        assert_eq!(obj_simple.status, FormatSupportStatus::PreviewSupported);

        let ply = gate_source_format("simple.ply", PLY_SIMPLE, &limits).unwrap();
        assert_eq!(ply.status, FormatSupportStatus::PreviewSupported);

        let glb = gate_source_format("simple.glb", GLB_SIMPLE, &limits).unwrap();
        assert_eq!(glb.status, FormatSupportStatus::PreviewSupported);

        let gltf = gate_source_format("embedded.gltf", GLTF_EMBEDDED, &limits).unwrap();
        assert_eq!(gltf.status, FormatSupportStatus::PreviewSupported);
        assert_eq!(
            gate_source_format("external.gltf", GLTF_EXTERNAL, &limits).unwrap_err(),
            PreviewError::ExternalReferenceDisabled
        );

        let three_mf = gate_source_format("simple_model.3mf", THREEMF_SIMPLE, &limits).unwrap();
        assert_eq!(three_mf.source_format, SourceFormat::ThreeMf);
        assert_eq!(three_mf.status, FormatSupportStatus::ManualReviewRequired);
    }

    #[test]
    fn glb_gate_rejects_external_uri_in_json_chunk() {
        let json = serde_json::json!({
            "asset": {"version": "2.0"},
            "buffers": [{"uri": "model.bin", "byteLength": 4}],
        });
        let glb = gltf::binary::Glb {
            header: gltf::binary::Header {
                magic: *b"glTF",
                version: 2,
                length: 0,
            },
            json: serde_json::to_vec(&json).unwrap().into(),
            bin: None,
        };
        let bytes = glb.to_vec().unwrap();
        assert_eq!(
            validate_glb(&bytes).unwrap_err(),
            PreviewError::ExternalReferenceDisabled
        );
    }

    #[test]
    fn compressed_3mf_relationship_entries_are_scanned_for_external_targets() {
        let body = br#"<Relationships><Relationship TargetMode="External" Target="http://example.com"/></Relationships>"#;
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(body).unwrap();
        let compressed = encoder.finish().unwrap();
        let name = b"_rels/.rels";
        let mut entry = Vec::new();
        entry.extend_from_slice(b"PK\x03\x04");
        entry.extend_from_slice(&20_u16.to_le_bytes());
        entry.extend_from_slice(&0_u16.to_le_bytes());
        entry.extend_from_slice(&8_u16.to_le_bytes());
        entry.extend_from_slice(&0_u16.to_le_bytes());
        entry.extend_from_slice(&0_u16.to_le_bytes());
        entry.extend_from_slice(&0_u32.to_le_bytes());
        entry.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        entry.extend_from_slice(&(body.len() as u32).to_le_bytes());
        entry.extend_from_slice(&(name.len() as u16).to_le_bytes());
        entry.extend_from_slice(&0_u16.to_le_bytes());
        entry.extend_from_slice(name);
        entry.extend_from_slice(&compressed);
        assert_eq!(
            inspect_relationship_entry(&entry, 0, 8, compressed.len()).unwrap_err(),
            PreviewError::ExternalReferenceDisabled
        );
    }

    #[test]
    fn preview_conversion_covers_supported_smoke_formats_through_unified_glb_output() {
        let limits = ConversionLimits::default();
        let three_mf_limits = ThreeMfLimits::default();
        for (filename, bytes, expected_format) in [
            ("single_cube.obj", OBJ_SINGLE_CUBE, SourceFormat::Obj),
            ("simple.ply", PLY_SIMPLE, SourceFormat::Ply),
            ("simple.glb", GLB_SIMPLE, SourceFormat::Glb),
            (
                "embedded_triangle.gltf",
                GLTF_EMBEDDED_TRIANGLE,
                SourceFormat::Gltf,
            ),
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
