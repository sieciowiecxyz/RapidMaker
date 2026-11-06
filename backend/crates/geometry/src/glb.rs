use std::{collections::HashMap, ops::Range};

use super::{MeshScene, PreviewAnalysis, PreviewError, gltf_number, pad_to_4};

const MAX_LOCAL_VERTICES: usize = 65_535;
const MAX_PRIMITIVES: usize = 64;
const MAX_JSON_BYTES: usize = 64 * 1024;

struct Primitive {
    vertices: Vec<u32>,
    indices: Range<usize>,
}

// Plan only bounded remapping metadata. The output buffer is allocated after
// both complete wire sizes have been checked, including JSON and alignment.
fn compact_primitives(scene: &MeshScene) -> Option<Vec<Primitive>> {
    let mut primitives = Vec::new();
    let mut remap = HashMap::<u32, u16>::new();
    let mut current = Primitive {
        vertices: Vec::new(),
        indices: 0..0,
    };
    for (triangle, indices) in scene.indices.chunks_exact(3).enumerate() {
        let new_vertices = indices
            .iter()
            .enumerate()
            .filter(|(i, index)| !remap.contains_key(index) && !indices[..*i].contains(index))
            .count();
        if remap.len() + new_vertices > MAX_LOCAL_VERTICES {
            primitives.push(current);
            if primitives.len() == MAX_PRIMITIVES {
                return None;
            }
            remap.clear();
            current = Primitive {
                vertices: Vec::new(),
                indices: triangle * 3..triangle * 3,
            };
        }
        for &index in indices {
            let next = remap.len() as u16;
            if let std::collections::hash_map::Entry::Vacant(entry) = remap.entry(index) {
                entry.insert(next);
                current.vertices.push(index);
            }
        }
        current.indices.end += 3;
    }
    primitives.push(current);
    Some(primitives)
}

fn compact_layout(
    scene: &MeshScene,
    primitives: &[Primitive],
    node_transform: &str,
) -> Result<(String, usize), PreviewError> {
    use std::fmt::Write;
    let mut views = String::new();
    let mut accessors = String::new();
    let mut meshes = String::new();
    let mut offset = 0usize;
    for (i, primitive) in primitives.iter().enumerate() {
        let position_len = primitive.vertices.len() * 12;
        let index_len = primitive
            .indices
            .len()
            .checked_mul(2)
            .ok_or(PreviewError::OutputTooLarge)?;
        let index_offset = offset
            .checked_add(position_len)
            .ok_or(PreviewError::OutputTooLarge)?;
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for &index in &primitive.vertices {
            for axis in 0..3 {
                let value = scene.positions_mm[index as usize][axis];
                min[axis] = min[axis].min(value);
                max[axis] = max[axis].max(value);
            }
        }
        let a = i * 2;
        let b = a + 1;
        let comma = if i == 0 { "" } else { "," };
        write!(
            meshes,
            "{comma}{{\"attributes\":{{\"POSITION\":{a}}},\"indices\":{b}}}"
        )
        .unwrap();
        write!(views, "{comma}{{\"buffer\":0,\"byteOffset\":{offset},\"byteLength\":{position_len},\"target\":34962}},{{\"buffer\":0,\"byteOffset\":{index_offset},\"byteLength\":{index_len},\"target\":34963}}").unwrap();
        write!(accessors, "{comma}{{\"bufferView\":{a},\"componentType\":5126,\"count\":{},\"type\":\"VEC3\",\"min\":[{},{},{}],\"max\":[{},{},{}]}},{{\"bufferView\":{b},\"componentType\":5123,\"count\":{},\"type\":\"SCALAR\"}}", primitive.vertices.len(), gltf_number(min[0]), gltf_number(min[1]), gltf_number(min[2]), gltf_number(max[0]), gltf_number(max[1]), gltf_number(max[2]), primitive.indices.len()).unwrap();
        offset = index_offset
            .checked_add(index_len)
            .and_then(|n| n.checked_add(3))
            .ok_or(PreviewError::OutputTooLarge)?
            & !3;
    }
    Ok((
        format!(
            "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"rapidmaker-geometry\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],\"nodes\":[{{\"mesh\":0{node_transform}}}],\"meshes\":[{{\"primitives\":[{meshes}]}}],\"buffers\":[{{\"byteLength\":{offset}}}],\"bufferViews\":[{views}],\"accessors\":[{accessors}]}}"
        ),
        offset,
    ))
}

fn total_len(json: &str, buffer_len: usize) -> Option<usize> {
    if json.len() > MAX_JSON_BYTES {
        return None;
    }
    let json_len = json.len().checked_add(3)? & !3;
    json_len
        .checked_add(28)?
        .checked_add(buffer_len)
        .filter(|n| *n <= u32::MAX as usize)
}

pub(super) fn write_glb_bounded(
    scene: &MeshScene,
    analysis: &PreviewAnalysis,
    max_bytes: usize,
    node_transform: &str,
) -> Result<Vec<u8>, PreviewError> {
    match write_glb_ordered(scene, analysis, max_bytes, node_transform) {
        Err(PreviewError::OutputTooLarge) if scene.indices.len() <= max_bytes / 2 => {
            minimum_encoded_bytes(scene)
                .filter(|&bytes| bytes <= max_bytes)
                .ok_or(PreviewError::OutputTooLarge)?;
            let reordered = spatial_order(scene);
            write_glb_ordered(&reordered, analysis, max_bytes, node_transform)
        }
        result => result,
    }
}

// Index validation has already succeeded; unused global positions cost no compact bytes.
fn minimum_encoded_bytes(scene: &MeshScene) -> Option<usize> {
    let mut seen = vec![false; scene.positions_mm.len()];
    let mut referenced = 0usize;
    for &index in &scene.indices {
        if !seen[index as usize] {
            seen[index as usize] = true;
            referenced += 1;
        }
    }
    referenced
        .checked_mul(12)?
        .checked_add(scene.indices.len().checked_mul(2)?)?
        .checked_add(28)
}

// Quantize only sorting keys: positions and each oriented triangle remain exact.
// f64 arithmetic keeps spans/centroids finite even for opposite f32 extremes.
fn spatial_order(scene: &MeshScene) -> MeshScene {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for point in &scene.positions_mm {
        for axis in 0..3 {
            let value = f64::from(point[axis]);
            min[axis] = min[axis].min(value);
            max[axis] = max[axis].max(value);
        }
    }
    let mut triangles = scene
        .indices
        .chunks_exact(3)
        .map(|indices| {
            let triangle = [indices[0], indices[1], indices[2]];
            let mut key = 0;
            for axis in 0..3 {
                let center = triangle
                    .iter()
                    .map(|&i| f64::from(scene.positions_mm[i as usize][axis]))
                    .sum::<f64>()
                    / 3.0;
                let span = max[axis] - min[axis];
                let normalized = if span > 0.0 {
                    (center - min[axis]) / span
                } else {
                    0.0
                };
                let mut bits = (normalized.clamp(0.0, 1.0) * 1023.0) as u32;
                bits = (bits | bits << 16) & 0x0300_00ff;
                bits = (bits | bits << 8) & 0x0300_f00f;
                bits = (bits | bits << 4) & 0x030c_30c3;
                bits = (bits | bits << 2) & 0x0924_9249;
                key |= bits << axis;
            }
            (key, triangle)
        })
        .collect::<Vec<_>>();
    triangles.sort_unstable_by_key(|(key, _)| *key);
    MeshScene {
        positions_mm: scene.positions_mm.clone(),
        indices: triangles
            .into_iter()
            .flat_map(|(_, triangle)| triangle)
            .collect(),
    }
}

fn write_glb_ordered(
    scene: &MeshScene,
    analysis: &PreviewAnalysis,
    max_bytes: usize,
    node_transform: &str,
) -> Result<Vec<u8>, PreviewError> {
    if scene.positions_mm.is_empty() || scene.indices.is_empty() {
        return Err(PreviewError::EmptyMesh);
    }
    if !scene.indices.len().is_multiple_of(3)
        || scene
            .indices
            .iter()
            .any(|&i| i as usize >= scene.positions_mm.len())
    {
        return Err(PreviewError::MalformedInput);
    }
    if scene
        .positions_mm
        .iter()
        .flatten()
        .chain(&analysis.bounds_min_mm)
        .chain(&analysis.bounds_max_mm)
        .any(|v| !v.is_finite())
    {
        return Err(PreviewError::NonFiniteCoordinate);
    }
    let legacy = legacy_layout(scene, analysis, node_transform)?;
    let legacy_len = total_len(&legacy.0, legacy.1).unwrap_or(usize::MAX);
    // Even the smallest possible compact representation needs two bytes per index.
    if scene.indices.len() > max_bytes / 2 {
        return Err(PreviewError::OutputTooLarge);
    }
    let compact = compact_primitives(scene).and_then(|primitives| {
        let layout = compact_layout(scene, &primitives, node_transform).ok()?;
        let size = total_len(&layout.0, layout.1)?;
        (size <= max_bytes && size < legacy_len).then_some((primitives, layout, size))
    });
    let (primitives, (json, buffer_len), size) = match compact {
        Some((primitives, layout, size)) => (Some(primitives), layout, size),
        None if legacy_len <= max_bytes => (None, legacy, legacy_len),
        None => return Err(PreviewError::OutputTooLarge),
    };
    let mut json = json.into_bytes();
    pad_to_4(&mut json, b' ');
    let mut glb = Vec::with_capacity(size);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(size as u32).to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json);
    glb.extend_from_slice(&(buffer_len as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    if let Some(primitives) = primitives {
        for primitive in primitives {
            let mut remap = HashMap::with_capacity(primitive.vertices.len());
            for (local, &global) in primitive.vertices.iter().enumerate() {
                remap.insert(global, local as u16);
                for value in scene.positions_mm[global as usize] {
                    glb.extend_from_slice(&value.to_le_bytes());
                }
            }
            for &index in &scene.indices[primitive.indices] {
                glb.extend_from_slice(&remap[&index].to_le_bytes());
            }
            pad_to_4(&mut glb, 0);
        }
    } else {
        for vertex in &scene.positions_mm {
            for value in vertex {
                glb.extend_from_slice(&value.to_le_bytes());
            }
        }
        for index in &scene.indices {
            glb.extend_from_slice(&index.to_le_bytes());
        }
    }
    debug_assert_eq!(glb.len(), size);
    Ok(glb)
}

pub fn write_glb(scene: &MeshScene, analysis: &PreviewAnalysis) -> Result<Vec<u8>, PreviewError> {
    write_glb_bounded(scene, analysis, u32::MAX as usize, "")
}

fn legacy_layout(
    scene: &MeshScene,
    analysis: &PreviewAnalysis,
    node_transform: &str,
) -> Result<(String, usize), PreviewError> {
    let positions_len = scene
        .positions_mm
        .len()
        .checked_mul(12)
        .ok_or(PreviewError::OutputTooLarge)?;
    let indices_len = scene
        .indices
        .len()
        .checked_mul(4)
        .ok_or(PreviewError::OutputTooLarge)?;
    let buffer_len = positions_len
        .checked_add(indices_len)
        .ok_or(PreviewError::OutputTooLarge)?;
    let json = format!(
        concat!(
            "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"rapidmaker-geometry\"}},",
            "\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],\"nodes\":[{{\"mesh\":0{node_transform}}}],",
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
        node_transform = node_transform,
        buffer_len = buffer_len,
        positions_len = positions_len,
        index_offset = positions_len,
        indices_len = indices_len,
        vertex_count = scene.positions_mm.len(),
        index_count = scene.indices.len(),
        min_x = gltf_number(analysis.bounds_min_mm[0]),
        min_y = gltf_number(analysis.bounds_min_mm[1]),
        min_z = gltf_number(analysis.bounds_min_mm[2]),
        max_x = gltf_number(analysis.bounds_max_mm[0]),
        max_y = gltf_number(analysis.bounds_max_mm[1]),
        max_z = gltf_number(analysis.bounds_max_mm[2]),
    );
    Ok((json, buffer_len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConversionLimits, SourceFormat, analyze_scene};

    fn scene(count: u32, indices: Vec<u32>) -> MeshScene {
        MeshScene {
            positions_mm: (0..count)
                .map(|i| [i as f32, -0.0, (i % 7) as f32])
                .collect(),
            indices,
        }
    }

    fn analysis(scene: &MeshScene) -> PreviewAnalysis {
        analyze_scene(scene, SourceFormat::StlBinary, &ConversionLimits::default()).unwrap()
    }

    fn assert_exact_triangles(bytes: &[u8], scene: &MeshScene) {
        let gltf = gltf::Gltf::from_slice(bytes).unwrap();
        assert_exact_gltf_triangles(&gltf, scene);
    }

    fn assert_exact_gltf_triangles(gltf: &gltf::Gltf, scene: &MeshScene) {
        assert_eq!(gltf.meshes().count(), 1);
        assert_eq!(gltf.nodes().count(), 1);
        assert_eq!(gltf.buffers().count(), 1);
        let mut actual = Vec::new();
        for primitive in gltf.meshes().next().unwrap().primitives() {
            let reader = primitive.reader(|_| gltf.blob.as_deref());
            let positions = reader.read_positions().unwrap().collect::<Vec<_>>();
            let accessor = primitive.get(&gltf::Semantic::Positions).unwrap();
            let mut min = [f32::INFINITY; 3];
            let mut max = [f32::NEG_INFINITY; 3];
            for p in &positions {
                for axis in 0..3 {
                    min[axis] = min[axis].min(p[axis]);
                    max[axis] = max[axis].max(p[axis]);
                }
            }
            for (actual, expected) in [
                (accessor.min().unwrap(), min),
                (accessor.max().unwrap(), max),
            ] {
                for (value, expected) in actual.as_array().unwrap().iter().zip(expected) {
                    assert_eq!(value.as_f64().unwrap(), f64::from(expected));
                }
            }
            for index in reader.read_indices().unwrap().into_u32() {
                actual.push(positions[index as usize].map(f32::to_bits));
            }
        }
        let expected = scene
            .indices
            .iter()
            .map(|&i| scene.positions_mm[i as usize].map(f32::to_bits))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    fn assert_same_oriented_faces(bytes: &[u8], scene: &MeshScene) {
        let gltf = gltf::Gltf::from_slice(bytes).unwrap();
        let mut actual = Vec::new();
        for primitive in gltf.meshes().next().unwrap().primitives() {
            let reader = primitive.reader(|_| gltf.blob.as_deref());
            let positions = reader.read_positions().unwrap().collect::<Vec<_>>();
            let accessor = primitive.get(&gltf::Semantic::Positions).unwrap();
            for (bound, minimum) in [
                (accessor.min().unwrap(), true),
                (accessor.max().unwrap(), false),
            ] {
                for axis in 0..3 {
                    let values = positions.iter().map(|point| f64::from(point[axis]));
                    let expected = if minimum {
                        values.fold(f64::INFINITY, f64::min)
                    } else {
                        values.fold(f64::NEG_INFINITY, f64::max)
                    };
                    assert_eq!(bound[axis].as_f64().unwrap(), expected);
                }
            }
            let indices = reader
                .read_indices()
                .unwrap()
                .into_u32()
                .collect::<Vec<_>>();
            assert!(indices.len().is_multiple_of(3));
            actual.extend(indices.chunks_exact(3).map(|face| {
                [face[0], face[1], face[2]].map(|i| positions[i as usize].map(f32::to_bits))
            }));
        }
        let mut expected = scene
            .indices
            .chunks_exact(3)
            .map(|face| {
                [face[0], face[1], face[2]]
                    .map(|i| scene.positions_mm[i as usize].map(f32::to_bits))
            })
            .collect::<Vec<_>>();
        actual.sort_unstable();
        expected.sort_unstable();
        assert_eq!(actual.len(), scene.indices.len() / 3);
        assert!(actual == expected, "oriented face multiset changed");
    }

    #[test]
    fn spatial_lower_bound_counts_only_referenced_positions_and_rejects_irreducible_bytes() {
        let scene = scene(4096, vec![0, 1, 2, 0, 1, 2]);
        let analysis = analysis(&scene);
        let minimum = 3 * 12 + 6 * 2 + 28;
        assert_eq!(minimum_encoded_bytes(&scene), Some(minimum));
        // Thousands of unused vertices must not make a small compact preview fail.
        let compact = write_glb_bounded(&scene, &analysis, 1024, "").unwrap();
        assert_same_oriented_faces(&compact, &scene);
        assert_eq!(
            write_glb_bounded(&scene, &analysis, minimum - 1, "").err(),
            Some(PreviewError::OutputTooLarge)
        );
    }

    #[test]
    fn spatial_fallback_fits_interleaved_islands_without_losing_or_reorienting_faces() {
        let bank = 40_002u32;
        let mut scene = scene(2 * bank, Vec::new());
        for (i, point) in scene.positions_mm.iter_mut().enumerate() {
            point[0] = (i as u32 / bank * 200_000 + i as u32 % bank) as f32;
        }
        for _ in 0..3 {
            for triangle in 0..bank / 3 {
                for base in [0, bank] {
                    let start = base + triangle * 3;
                    scene.indices.extend([start, start + 1, start + 2]);
                }
            }
        }
        scene.indices.extend([0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
        let original_indices = scene.indices.clone();
        let original_bits = scene
            .positions_mm
            .iter()
            .map(|p| p.map(f32::to_bits))
            .collect::<Vec<_>>();
        let analysis = analysis(&scene);
        let fitting = write_glb_bounded(&scene, &analysis, u32::MAX as usize, "").unwrap();
        assert_eq!(
            fitting,
            write_glb_ordered(&scene, &analysis, u32::MAX as usize, "").unwrap()
        );
        assert_exact_triangles(&fitting, &scene);
        assert_eq!(
            write_glb_ordered(&scene, &analysis, 1_600_000, "").err(),
            Some(PreviewError::OutputTooLarge)
        );
        let fallback = write_glb_bounded(&scene, &analysis, 1_600_000, "").unwrap();
        assert!(fallback.len() < fitting.len());
        assert_same_oriented_faces(&fallback, &scene);
        assert_eq!(
            write_glb_bounded(&scene, &analysis, fallback.len(), "").unwrap(),
            fallback
        );
        assert_eq!(
            write_glb_bounded(&scene, &analysis, fallback.len() - 1, "").err(),
            Some(PreviewError::OutputTooLarge)
        );
        assert_eq!(scene.indices, original_indices);
        assert_eq!(
            scene
                .positions_mm
                .iter()
                .map(|p| p.map(f32::to_bits))
                .collect::<Vec<_>>(),
            original_bits
        );
    }

    #[test]
    fn spatial_centroids_remain_finite_near_f32_extremes_and_keep_degenerates() {
        let high = 2.0f32.powi(127);
        let scene = MeshScene {
            positions_mm: vec![
                [high, -0., 0.1],
                [high, 0., high],
                [high, -0., -high / 2.],
                [-high / 2., 0., 0.33333334],
            ],
            indices: vec![0, 1, 2, 3, 3, 3, 0, 1, 2, 2, 1, 0],
        };
        let limits = ConversionLimits {
            max_coordinate_abs_mm: f32::MAX,
            ..Default::default()
        };
        let analysis = analyze_scene(&scene, SourceFormat::StlBinary, &limits).unwrap();
        let reordered = spatial_order(&scene);
        let glb = write_glb_ordered(&reordered, &analysis, 64 * 1024, "").unwrap();
        assert_same_oriented_faces(&glb, &scene);
    }

    #[test]
    fn primitive_budget_accepts_64_and_falls_back_before_65() {
        let mut indices = Vec::with_capacity(64 * MAX_LOCAL_VERTICES + 3);
        for bank in 0..MAX_PRIMITIVES {
            let start = ((bank % 2) * MAX_LOCAL_VERTICES) as u32;
            indices.extend(start..start + MAX_LOCAL_VERTICES as u32);
        }
        let mut scene = scene((2 * MAX_LOCAL_VERTICES) as u32, indices);
        let primitives = compact_primitives(&scene).unwrap();
        assert_eq!(primitives.len(), MAX_PRIMITIVES);
        assert_eq!(primitives.last().unwrap().indices.end, scene.indices.len());
        let transform = r#","scale":[0.001,0.001,0.001],"rotation":[-0.70710678,0,0,0.70710678]"#;
        let (json, buffer_len) = compact_layout(&scene, &primitives, transform).unwrap();
        assert!(json.len() <= MAX_JSON_BYTES);
        assert!(total_len(&json, buffer_len).is_some());
        drop(primitives);
        // The last primitive uses bank B: these bank-A indices require #65.
        scene.indices.extend([0, 1, 2]);
        assert!(compact_primitives(&scene).is_none());
    }

    #[test]
    fn fractional_bounds_match_promoted_binary_positions_in_both_encodings() {
        let scene = MeshScene {
            positions_mm: vec![
                [0.1, 10.2, -123.456],
                [0.33333334, -0.1, 10.2],
                [-123.456, 0.33333334, 0.1],
            ],
            indices: vec![0, 1, 2],
        };
        let analysis = analysis(&scene);
        let compact = write_glb(&scene, &analysis).unwrap();
        assert_exact_triangles(&compact, &scene);
        let parsed = gltf::Gltf::from_slice(&compact).unwrap();
        assert_eq!(
            parsed.accessors().nth(1).unwrap().data_type(),
            gltf::accessor::DataType::U16
        );

        // Parse the legacy JSON through the independent glTF reader, supplying
        // its u32 binary buffer so bounds are compared with decoded positions.
        let (json, buffer_len) = legacy_layout(&scene, &analysis, "").unwrap();
        let mut legacy = gltf::Gltf::from_slice(json.as_bytes()).unwrap();
        let mut binary = Vec::with_capacity(buffer_len);
        for value in scene.positions_mm.iter().flatten() {
            binary.extend_from_slice(&value.to_le_bytes());
        }
        for index in &scene.indices {
            binary.extend_from_slice(&index.to_le_bytes());
        }
        legacy.blob = Some(binary);
        assert_exact_gltf_triangles(&legacy, &scene);
    }

    #[test]
    fn compact_boundary_padding_and_transform_preserve_every_triangle_bit() {
        let mut indices = (0..65_535).collect::<Vec<_>>();
        indices.extend([65_534, 65_535, 65_536, 0, 0, 0]);
        let scene = scene(65_537, indices);
        let analysis = analysis(&scene);
        let transform = r#","scale":[0.001,0.001,0.001],"rotation":[-0.70710678,0,0,0.70710678]"#;
        let bytes = write_glb_bounded(&scene, &analysis, usize::MAX, transform).unwrap();
        assert_exact_triangles(&bytes, &scene);
        let gltf = gltf::Gltf::from_slice(&bytes).unwrap();
        let primitives = gltf
            .meshes()
            .next()
            .unwrap()
            .primitives()
            .collect::<Vec<_>>();
        assert_eq!(primitives.len(), 2);
        assert_eq!(
            primitives[0]
                .get(&gltf::Semantic::Positions)
                .unwrap()
                .count(),
            65_535
        );
        for primitive in &primitives {
            assert_eq!(
                primitive.indices().unwrap().data_type(),
                gltf::accessor::DataType::U16
            );
            let reader = primitive.reader(|_| gltf.blob.as_deref());
            assert!(
                reader
                    .read_indices()
                    .unwrap()
                    .into_u32()
                    .all(|i| i < 65_535)
            );
        }
        let views = gltf.views().collect::<Vec<_>>();
        assert_eq!(views.len(), 4);
        assert_eq!(views[1].length(), 65_535 * 2);
        assert_eq!(views[2].offset(), views[1].offset() + views[1].length() + 2);
        let blob = gltf.blob.as_ref().unwrap();
        assert_eq!(
            &blob[views[1].offset() + views[1].length()..views[2].offset()],
            &[0, 0]
        );
        let (_, rotation, scale) = gltf.nodes().next().unwrap().transform().decomposed();
        assert_eq!(scale, [0.001; 3]);
        assert!((rotation[0] + std::f32::consts::FRAC_1_SQRT_2).abs() < 0.00001);
        assert_eq!(
            write_glb_bounded(&scene, &analysis, bytes.len(), transform).unwrap(),
            bytes
        );
        assert_eq!(
            write_glb_ordered(&scene, &analysis, bytes.len() - 1, transform).unwrap_err(),
            PreviewError::OutputTooLarge
        );
        // Below the original wire size, the public writer may reorder whole faces.
        let fallback = write_glb_bounded(&scene, &analysis, bytes.len() - 1, transform).unwrap();
        assert!(fallback.len() < bytes.len());
        assert_same_oriented_faces(&fallback, &scene);
        let reordered = gltf::Gltf::from_slice(&fallback).unwrap();
        assert_eq!(
            reordered.nodes().next().unwrap().transform().matrix(),
            gltf.nodes().next().unwrap().transform().matrix()
        );
        let legacy = legacy_layout(&scene, &analysis, transform).unwrap();
        assert!(bytes.len() < total_len(&legacy.0, legacy.1).unwrap());
    }

    #[test]
    fn vertex_duplication_overhead_falls_back_to_legacy_u32() {
        let mut indices = (0..65_535).collect::<Vec<_>>();
        indices.extend([65_535, 65_536, 0]);
        indices.extend(1..65_533);
        let scene = scene(65_537, indices);
        let analysis = analysis(&scene);
        let bytes = write_glb(&scene, &analysis).unwrap();
        assert_exact_triangles(&bytes, &scene);
        let gltf = gltf::Gltf::from_slice(&bytes).unwrap();
        assert_eq!(gltf.meshes().next().unwrap().primitives().count(), 1);
        assert_eq!(
            gltf.meshes()
                .next()
                .unwrap()
                .primitives()
                .next()
                .unwrap()
                .indices()
                .unwrap()
                .data_type(),
            gltf::accessor::DataType::U32
        );
        let legacy = legacy_layout(&scene, &analysis, "").unwrap();
        assert_eq!(bytes.len(), total_len(&legacy.0, legacy.1).unwrap());
        assert_eq!(
            write_glb_bounded(&scene, &analysis, bytes.len(), "").unwrap(),
            bytes
        );
        assert_eq!(
            write_glb_ordered(&scene, &analysis, bytes.len() - 1, "").unwrap_err(),
            PreviewError::OutputTooLarge
        );
        let fallback = write_glb_bounded(&scene, &analysis, bytes.len() - 1, "").unwrap();
        assert!(fallback.len() < bytes.len());
        assert_same_oriented_faces(&fallback, &scene);
    }

    #[test]
    fn invalid_indices_return_error_without_panicking() {
        let mut scene = scene(3, vec![0, 1, 2]);
        let analysis = analysis(&scene);
        scene.indices[2] = u32::MAX;
        assert_eq!(
            write_glb(&scene, &analysis).unwrap_err(),
            PreviewError::MalformedInput
        );
        scene.indices = vec![0, 1];
        assert_eq!(
            write_glb(&scene, &analysis).unwrap_err(),
            PreviewError::MalformedInput
        );
    }
}
