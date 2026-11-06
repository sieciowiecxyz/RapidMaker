//! Bounded display-only approximation. Canonical geometry never enters this module mutably.
use meshopt::{SimplifyOptions, VertexDataAdapter};

use super::{
    MeshScene, PreviewAnalysis, PreviewApproximation, PreviewError, glb::write_glb_bounded,
};

const MAX_BYTES: usize = 25 * 1024 * 1024;
const MAX_TRIANGLES: usize = 2_000_000;
const TARGET_TRIANGLES: usize = 1_700_000;
const ERROR_MM: f32 = 0.05;

pub(super) fn write(
    scene: &MeshScene,
    analysis: &PreviewAnalysis,
    max_bytes: usize,
    transform: &str,
) -> Result<(Vec<u8>, Option<PreviewApproximation>), PreviewError> {
    // A larger diagnostic allowance explicitly requests the original exact mesh.
    if max_bytes > MAX_BYTES || analysis.triangle_count <= MAX_TRIANGLES {
        match write_glb_bounded(scene, analysis, max_bytes, transform) {
            Ok(bytes) => return Ok((bytes, None)),
            Err(PreviewError::OutputTooLarge) if max_bytes <= MAX_BYTES => {}
            Err(error) => return Err(error),
        }
    }
    let target = TARGET_TRIANGLES.min(analysis.triangle_count.saturating_mul(3) / 4);
    approximate(scene, analysis, max_bytes, transform, target)
}

fn approximate(
    scene: &MeshScene,
    analysis: &PreviewAnalysis,
    max_bytes: usize,
    transform: &str,
    target: usize,
) -> Result<(Vec<u8>, Option<PreviewApproximation>), PreviewError> {
    if scene.indices.is_empty()
        || !scene.indices.len().is_multiple_of(3)
        || scene
            .indices
            .iter()
            .any(|&i| i as usize >= scene.positions_mm.len())
        || scene.positions_mm.iter().flatten().any(|v| !v.is_finite())
    {
        return Err(PreviewError::MalformedInput);
    }
    if scene.indices.len() / 3 <= target || max_bytes < 32 {
        return Err(PreviewError::OutputTooLarge);
    }
    let mut components: Vec<u32> = (0..scene.positions_mm.len() as u32).collect();
    for triangle in scene.indices.chunks_exact(3) {
        join(&mut components, triangle);
    }
    for i in 0..components.len() {
        components[i] = root(&mut components, i as u32);
    }
    let mut present = vec![false; components.len()];
    let mut locks = vec![false; components.len()];
    for triangle in scene.indices.chunks_exact(3) {
        let component = components[triangle[0] as usize] as usize;
        if !present[component] {
            // Keep a face-sized anchor in every component, including tiny detached parts.
            for &i in triangle {
                locks[i as usize] = true;
            }
            present[component] = true;
        }
    }
    for axis in 0..3 {
        for value in [analysis.bounds_min_mm[axis], analysis.bounds_max_mm[axis]] {
            let triangle = scene
                .indices
                .chunks_exact(3)
                .find(|triangle| {
                    triangle
                        .iter()
                        .any(|&i| scene.positions_mm[i as usize][axis] == value)
                })
                .ok_or(PreviewError::OutputTooLarge)?;
            // Lock a whole incident face: an isolated locked vertex may otherwise
            // disappear when all its adjacent triangles collapse.
            for &index in triangle {
                locks[index as usize] = true;
            }
        }
    }
    let adapter = VertexDataAdapter::new(meshopt::typed_to_bytes(&scene.positions_mm), 12, 0)
        .map_err(|_| PreviewError::MalformedInput)?;
    let mut error = 0.0;
    let mut indices = meshopt::simplify_with_locks(
        &scene.indices,
        &adapter,
        &locks,
        target.min(TARGET_TRIANGLES) * 3,
        ERROR_MM,
        SimplifyOptions::LockBorder | SimplifyOptions::ErrorAbsolute,
        Some(&mut error),
    );
    // The target is advisory. Failure to fit never increases the error tolerance.
    if !error.is_finite()
        || !(0.0..=ERROR_MM).contains(&error)
        || indices.is_empty()
        || !indices.len().is_multiple_of(3)
        || indices.len() / 3 > MAX_TRIANGLES
        || indices.len() >= scene.indices.len()
    {
        return Err(PreviewError::OutputTooLarge);
    }
    validate_components(&indices, &components, &present)?;
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for &index in &indices {
        let point = scene
            .positions_mm
            .get(index as usize)
            .ok_or(PreviewError::MalformedInput)?;
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    if min != analysis.bounds_min_mm || max != analysis.bounds_max_mm {
        return Err(PreviewError::OutputTooLarge);
    }
    let approximation = PreviewApproximation {
        source_triangles: analysis.triangle_count,
        display_triangles: indices.len() / 3,
        reported_error_mm: error,
        max_error_mm: ERROR_MM,
    };
    indices = meshopt::optimize_vertex_cache(&indices, scene.positions_mm.len());
    let display = MeshScene {
        positions_mm: meshopt::optimize_vertex_fetch(&mut indices, &scene.positions_mm),
        indices,
    };
    let mut display_analysis = analysis.clone();
    display_analysis.vertex_count = display.positions_mm.len();
    display_analysis.triangle_count = approximation.display_triangles;
    // Self-contained metadata; no change to exact outputs. Values are validated numbers.
    let transform = format!(
        "{transform},\"extras\":{{\"rapidmakerPreview\":{{\"source_triangles\":{},\"display_triangles\":{},\"reported_error_mm\":{},\"max_error_mm\":{ERROR_MM}}}}}",
        approximation.source_triangles, approximation.display_triangles, error,
    );
    let glb = write_glb_bounded(
        &display,
        &display_analysis,
        max_bytes.min(MAX_BYTES),
        &transform,
    )?;
    Ok((glb, Some(approximation)))
}

fn root(parents: &mut [u32], mut i: u32) -> u32 {
    while parents[i as usize] != i {
        parents[i as usize] = parents[parents[i as usize] as usize];
        i = parents[i as usize];
    }
    i
}

fn join(parents: &mut [u32], triangle: &[u32]) {
    let a = root(parents, triangle[0]);
    for &i in &triangle[1..] {
        let b = root(parents, i);
        parents[b as usize] = a;
    }
}

// Check one-to-one correspondence of vertex-connected components: no loss, merge or split.
fn validate_components(
    indices: &[u32],
    source: &[u32],
    present: &[bool],
) -> Result<(), PreviewError> {
    let mut output: Vec<u32> = (0..source.len() as u32).collect();
    for triangle in indices.chunks_exact(3) {
        let a = *source
            .get(triangle[0] as usize)
            .ok_or(PreviewError::MalformedInput)?;
        if triangle.iter().any(|&i| source.get(i as usize) != Some(&a)) {
            return Err(PreviewError::MalformedInput);
        }
        join(&mut output, triangle);
    }
    let mut mapping = vec![u32::MAX; source.len()];
    for &i in indices {
        let component = source[i as usize] as usize;
        let found = root(&mut output, i);
        if mapping[component] != u32::MAX && mapping[component] != found {
            return Err(PreviewError::OutputTooLarge);
        }
        mapping[component] = found;
    }
    if present
        .iter()
        .enumerate()
        .any(|(i, &used)| used && mapping[i] == u32::MAX)
    {
        return Err(PreviewError::OutputTooLarge);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ConversionLimits, SourceFormat, ThreeMfLimits, analyze_scene, source_to_production,
    };

    fn grid() -> MeshScene {
        let mut scene = MeshScene {
            positions_mm: Vec::new(),
            indices: Vec::new(),
        };
        // Two separated open sheets and a tiny detached triangle. Their boundaries
        // and the narrow raised feature must survive without joining the sheets.
        for part in 0..2 {
            let base = scene.positions_mm.len() as u32;
            for y in 0..=20 {
                for x in 0..=20 {
                    scene.positions_mm.push([
                        x as f32 * 0.1 + part as f32 * 4.0,
                        y as f32 * 0.1,
                        if x == 10 && y == 10 {
                            0.3
                        } else {
                            0.001 * (x as f32).sin()
                        },
                    ]);
                }
            }
            for y in 0..20u32 {
                for x in 0..20u32 {
                    let a = base + y * 21 + x;
                    scene
                        .indices
                        .extend_from_slice(&[a, a + 1, a + 22, a, a + 22, a + 21]);
                }
            }
        }
        let base = scene.positions_mm.len() as u32;
        scene.positions_mm.extend_from_slice(&[
            [9.0, 0.0, 0.0],
            [9.001, 0.0, 0.0],
            [9.0, 0.001, 0.0],
        ]);
        scene.indices.extend_from_slice(&[base, base + 1, base + 2]);
        scene
    }

    fn canonical_bytes(scene: &MeshScene) -> Vec<u8> {
        let mut bytes = vec![0; 80];
        bytes.extend_from_slice(&(scene.indices.len() as u32 / 3).to_le_bytes());
        for tri in scene.indices.chunks_exact(3) {
            bytes.extend_from_slice(&[0; 12]);
            for &i in tri {
                for value in scene.positions_mm[i as usize] {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
            bytes.extend_from_slice(&[0; 2]);
        }
        bytes
    }

    #[test]
    fn approximation_preserves_components_boundaries_extrema_and_canonical_source() {
        let scene = grid();
        let before = scene.clone();
        let input = canonical_bytes(&scene);
        let production_before = source_to_production(
            "sheets.stl",
            &input,
            &ConversionLimits::default(),
            &ThreeMfLimits::default(),
        )
        .unwrap();
        let analysis = analyze_scene(
            &scene,
            SourceFormat::StlBinary,
            &ConversionLimits::default(),
        )
        .unwrap();
        let transform = r#","scale":[0.001,0.001,0.001],"rotation":[-0.70710678,0,0,0.70710678]"#;
        let (bytes, approximation) =
            approximate(&scene, &analysis, MAX_BYTES, transform, 120).unwrap();
        let approximation = approximation.unwrap();
        assert!(approximation.display_triangles < approximation.source_triangles);
        assert!(approximation.reported_error_mm <= ERROR_MM);
        let document = gltf::Gltf::from_slice(&bytes).unwrap();
        let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + json_len]).unwrap();
        assert_eq!(
            json["nodes"][0]["scale"],
            serde_json::json!([0.001, 0.001, 0.001])
        );
        assert_eq!(
            json["nodes"][0]["extras"]["rapidmakerPreview"]["source_triangles"],
            analysis.triangle_count
        );
        assert_eq!(
            json["nodes"][0]["extras"]["rapidmakerPreview"]["display_triangles"],
            approximation.display_triangles
        );
        let mut positions = std::collections::HashSet::new();
        for primitive in document.meshes().next().unwrap().primitives() {
            let reader = primitive.reader(|_| document.blob.as_deref());
            for point in reader.read_positions().unwrap() {
                positions.insert(point.map(f32::to_bits));
            }
        }
        for (i, point) in scene.positions_mm.iter().enumerate() {
            let local = i % 441;
            if i >= 882
                || local / 21 == 0
                || local / 21 == 20
                || local % 21 == 0
                || local % 21 == 20
                || i == 220
            {
                assert!(
                    positions.contains(&point.map(f32::to_bits)),
                    "boundary/thin part vertex {i} lost"
                );
            }
        }
        assert_eq!(scene, before);
        assert_eq!(canonical_bytes(&scene), input);
        let production_after = source_to_production(
            "sheets.stl",
            &input,
            &ConversionLimits::default(),
            &ThreeMfLimits::default(),
        )
        .unwrap();
        assert_eq!(production_before.stl, production_after.stl);
        assert_eq!(production_before.glb, production_after.glb);
    }

    #[test]
    fn fitting_exact_output_remains_identical_and_impossible_budget_rejects() {
        let scene = grid();
        let analysis = analyze_scene(
            &scene,
            SourceFormat::StlBinary,
            &ConversionLimits::default(),
        )
        .unwrap();
        let expected = write_glb_bounded(&scene, &analysis, MAX_BYTES, "").unwrap();
        let (actual, approximation) = write(&scene, &analysis, MAX_BYTES, "").unwrap();
        assert_eq!(expected, actual);
        assert_eq!(approximation, None);
        assert_eq!(
            approximate(&scene, &analysis, 128, "", 120).unwrap_err(),
            PreviewError::OutputTooLarge
        );
    }

    #[test]
    fn byte_limited_mesh_below_target_can_still_be_simplified() {
        let scene = grid();
        let analysis = analyze_scene(
            &scene,
            SourceFormat::StlBinary,
            &ConversionLimits::default(),
        )
        .unwrap();
        assert!(analysis.triangle_count < TARGET_TRIANGLES);
        let exact = write_glb_bounded(&scene, &analysis, MAX_BYTES, "").unwrap();
        let budget = exact.len() * 9 / 10;
        let (bytes, approximation) = write(&scene, &analysis, budget, "").unwrap();
        assert!(bytes.len() <= budget);
        assert!(approximation.unwrap().display_triangles < analysis.triangle_count);
    }

    #[test]
    fn component_validation_rejects_loss_merge_split_and_invalid_indices() {
        let source = [0, 0, 0, 3, 3, 3];
        let present = [true, false, false, true, false, false];
        assert!(validate_components(&[0, 1, 2, 3, 4, 5], &source, &present).is_ok());
        assert!(validate_components(&[0, 1, 2], &source, &present).is_err());
        assert!(validate_components(&[0, 1, 3, 3, 4, 5], &source, &present).is_err());
        assert!(validate_components(&[0, 1, 2, 3, 4, 99], &source, &present).is_err());
        assert!(
            validate_components(
                &[0, 1, 2, 3, 4, 5],
                &[0; 6],
                &[true, false, false, false, false, false]
            )
            .is_err()
        );
    }
}
