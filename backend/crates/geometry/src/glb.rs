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
            write_glb_bounded(&scene, &analysis, bytes.len() - 1, transform).unwrap_err(),
            PreviewError::OutputTooLarge
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
            write_glb_bounded(&scene, &analysis, bytes.len() - 1, "").unwrap_err(),
            PreviewError::OutputTooLarge
        );
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
