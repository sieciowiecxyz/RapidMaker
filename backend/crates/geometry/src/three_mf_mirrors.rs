//! Normalize mirrors only in the private package supplied to lib3mf, then restore
//! the exact transforms after package validation. Secondary parts retain their
//! existing XML parser path. No source is changed.
use super::{BAMBU_TRIANGLE_SETTINGS, ConversionLimits, PreviewError, ThreeMfModel, check_timeout};
use quick_xml::{Reader, Writer, events::Event};
use std::collections::HashMap;
use std::time::Instant;

type Transform = [f64; 12];

// Roughly 16 MiB for the table, plus transient rehash storage; not a hard RAM cap.
// Exhausting this hint retains SDK validation.
const MAX_HINT_EDGES: usize = 524_288;
#[derive(Default)]
struct MeshTopologyHint {
    vertices: u32,
    edges: HashMap<(u32, u32), u8>,
    non_manifold: bool,
}

impl MeshTopologyHint {
    fn triangle(&mut self, element: &quick_xml::events::BytesStart<'_>, cap: usize) -> Option<()> {
        let mut indices = [None; 3];
        for attr in element.attributes() {
            let attr = attr.ok()?;
            if let Some(i) = ["v1", "v2", "v3"]
                .iter()
                .position(|key| attr.key.as_ref() == *key)
            {
                indices[i] = Some(attr.value.parse::<u32>().ok()?);
            }
        }
        let [a, b, c] = indices.map(|index| index.filter(|index| *index < self.vertices));
        let (a, b, c) = (a?, b?, c?);
        if a == b || b == c || c == a {
            return None;
        }
        if !self.non_manifold {
            for (a, b) in [(a, b), (b, c), (c, a)] {
                let edge = (a.min(b), a.max(b));
                if self.edges.len() >= cap && !self.edges.contains_key(&edge) {
                    return None;
                }
                let count = self.edges.entry(edge).or_default();
                *count += 1;
                if *count == 3 {
                    self.non_manifold = true;
                    break;
                }
            }
        }
        Some(())
    }
}

#[derive(Default)]
pub(super) struct Mirrors {
    active: bool,
    build: Vec<Option<(Transform, Transform)>>,
    objects: HashMap<usize, Vec<Option<(Transform, Transform)>>>,
}

impl Mirrors {
    pub(super) fn restore(self, model: &mut ThreeMfModel) -> Result<(), PreviewError> {
        if !self.active {
            return Ok(());
        }
        fn restore_items<'a>(
            saved: Vec<Option<(Transform, Transform)>>,
            parsed: impl ExactSizeIterator<Item = &'a mut Option<Transform>>,
        ) -> Result<(), PreviewError> {
            if saved.len() != parsed.len() {
                return Err(PreviewError::MalformedInput);
            }
            for (saved, parsed) in saved.into_iter().zip(parsed) {
                if let Some((original, normalized)) = saved {
                    if *parsed != Some(normalized) {
                        return Err(PreviewError::MalformedInput);
                    }
                    *parsed = Some(original);
                }
            }
            Ok(())
        }
        restore_items(
            self.build,
            model.build.items.iter_mut().map(|item| &mut item.transform),
        )?;
        let mut objects = self.objects;
        for object in &mut model.resources.objects {
            let saved = objects
                .remove(&object.id)
                .ok_or(PreviewError::MalformedInput)?;
            restore_items(
                saved,
                object.components.iter_mut().map(|item| &mut item.transform),
            )?;
        }
        if !objects.is_empty() {
            return Err(PreviewError::MalformedInput);
        }
        Ok(())
    }
}

pub(super) fn normalize(
    xml: &str,
    limits: &ConversionLimits,
    started: Instant,
    root_topology: bool,
) -> Result<(Option<String>, Mirrors), PreviewError> {
    check_timeout(started, limits)?;
    if !xml.contains("transform")
        && !BAMBU_TRIANGLE_SETTINGS
            .iter()
            .any(|attr| xml.contains(attr))
    {
        check_timeout(started, limits)?;
        return Ok((None, Mirrors::default()));
    }
    let mut mirrors = Mirrors::default();
    let mut reader = Reader::from_str(xml);
    // Allocate/copy XML only when an element actually changes.
    let mut writer: Option<Writer<Vec<u8>>> = None;
    let mut copied_until = 0;
    let mut events = 0usize;
    let mut stack = Vec::<String>::new();
    let mut object_id = None;
    let mut topology: Option<MeshTopologyHint> = None;
    let mut topology_unambiguous = false;
    loop {
        if events.is_multiple_of(1024) {
            check_timeout(started, limits)?;
        }
        events += 1;
        let before = reader.buffer_position() as usize;
        let event = reader
            .read_event()
            .map_err(|_| PreviewError::MalformedInput)?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Eof => break,
            Event::Start(ref element) | Event::Empty(ref element) => {
                let name = element.local_name();
                let name = name.as_ref();
                let at = |path: &[&str]| stack.iter().map(String::as_str).eq(path.iter().copied());
                let object = name == "object" && at(&["model", "resources"]);
                let build = name == "item" && at(&["model", "build"]);
                let component =
                    name == "component" && at(&["model", "resources", "object", "components"]);
                if object {
                    topology = None;
                    topology_unambiguous = root_topology && element.name().as_ref() == "object";
                } else if object_id.is_some() && topology_unambiguous {
                    if element.name().as_ref() != name {
                        topology_unambiguous = false;
                    } else {
                        match (stack.len(), name) {
                            (3, "mesh") => topology = Some(MeshTopologyHint::default()),
                            (4, "vertices" | "triangles") if stack[3] == "mesh" => {}
                            (5, "vertex")
                                if stack[3] == "mesh" && stack[4] == "vertices" && empty =>
                            {
                                if let Some(hint) = &mut topology {
                                    match hint.vertices.checked_add(1) {
                                        Some(count) => hint.vertices = count,
                                        None => topology = None,
                                    }
                                }
                            }
                            (5, "triangle")
                                if stack[3] == "mesh" && stack[4] == "triangles" && empty => {}
                            _ => topology_unambiguous = false,
                        }
                    }
                    if !topology_unambiguous {
                        topology = None;
                    }
                }
                let mut changed = false;
                let mut mirror = None;
                let mut id = None;
                for (index, attr) in element.attributes().enumerate() {
                    if index.is_multiple_of(256) {
                        check_timeout(started, limits)?;
                    }
                    let attr = attr.map_err(|_| PreviewError::MalformedInput)?;
                    if object && attr.key.as_ref() == "id" {
                        id = Some(
                            attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .map_err(|_| PreviewError::MalformedInput)?
                                .parse::<usize>()
                                .map_err(|_| PreviewError::MalformedInput)?,
                        );
                    }
                    if (build || component) && attr.key.as_ref() == "transform" {
                        let value = attr
                            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .map_err(|_| PreviewError::MalformedInput)?;
                        let values = value
                            .split_whitespace()
                            .map(str::parse::<f64>)
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(|_| PreviewError::MalformedInput)?;
                        let original: Transform = values
                            .try_into()
                            .map_err(|_| PreviewError::MalformedInput)?;
                        let [a, b, c, d, e, f, g, h, i, _, _, _] = original;
                        let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
                        if original.iter().any(|v| !v.is_finite()) || !det.is_finite() || det == 0.0
                        {
                            return Err(PreviewError::MalformedInput);
                        }
                        if det < 0.0 {
                            let mut normalized = original;
                            for index in [0, 3, 6] {
                                normalized[index] = -normalized[index];
                            }
                            mirror = Some((original, normalized));
                            changed = true;
                            mirrors.active = true;
                        }
                    }
                    if name == "triangle" && BAMBU_TRIANGLE_SETTINGS.contains(&attr.key.as_ref()) {
                        changed = true;
                    }
                }
                if name == "triangle"
                    && topology
                        .as_mut()
                        .is_some_and(|hint| hint.triangle(element, MAX_HINT_EDGES).is_none())
                {
                    topology = None;
                }
                if object {
                    let id = id.ok_or(PreviewError::MalformedInput)?;
                    if mirrors.objects.insert(id, Vec::new()).is_some() {
                        return Err(PreviewError::MalformedInput);
                    }
                    object_id = Some(id);
                }
                if build {
                    mirrors.build.push(mirror);
                }
                if component {
                    mirrors
                        .objects
                        .get_mut(&object_id.ok_or(PreviewError::MalformedInput)?)
                        .ok_or(PreviewError::MalformedInput)?
                        .push(mirror);
                }
                if changed {
                    let mut cleaned = element.to_owned();
                    cleaned.clear_attributes();
                    for (index, attr) in element.attributes().enumerate() {
                        if index.is_multiple_of(256) {
                            check_timeout(started, limits)?;
                        }
                        let attr = attr.map_err(|_| PreviewError::MalformedInput)?;
                        if name == "triangle"
                            && BAMBU_TRIANGLE_SETTINGS.contains(&attr.key.as_ref())
                        {
                            continue;
                        }
                        if let Some((_, normalized)) =
                            mirror.filter(|_| attr.key.as_ref() == "transform")
                        {
                            let value = normalized.map(|v| v.to_string()).join(" ");
                            cleaned.push_attribute(("transform", value.as_str()));
                        } else {
                            cleaned.push_attribute(attr);
                        }
                    }
                    let writer =
                        writer.get_or_insert_with(|| Writer::new(Vec::with_capacity(xml.len())));
                    writer
                        .get_mut()
                        .extend_from_slice(&xml.as_bytes()[copied_until..before]);
                    writer
                        .write_event(if empty {
                            Event::Empty(cleaned)
                        } else {
                            Event::Start(cleaned)
                        })
                        .map_err(|_| PreviewError::MalformedInput)?;
                    copied_until = reader.buffer_position() as usize;
                }
                if !empty {
                    stack.push(name.to_owned());
                } else if object {
                    object_id = None;
                }
            }
            Event::End(ref element) => {
                // SDK keeps the last mesh in an object. Earlier meshes may be overwritten.
                if stack.len() == 3
                    && element.local_name().as_ref() == "object"
                    && topology.as_ref().is_some_and(|hint| hint.non_manifold)
                {
                    return Err(PreviewError::NonManifoldMesh);
                }
                if stack.pop().as_deref() != Some(element.local_name().as_ref()) {
                    return Err(PreviewError::MalformedInput);
                }
                if element.local_name().as_ref() == "object" {
                    object_id = None;
                }
            }
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(PreviewError::MalformedInput);
    }
    check_timeout(started, limits)?;
    let xml = if let Some(mut writer) = writer {
        writer
            .get_mut()
            .extend_from_slice(&xml.as_bytes()[copied_until..]);
        Some(String::from_utf8(writer.into_inner()).map_err(|_| PreviewError::MalformedInput)?)
    } else {
        None
    };
    if !mirrors.active {
        mirrors = Mirrors::default();
    }
    Ok((xml, mirrors))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh(triangles: &str) -> String {
        format!(
            r#"<mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles>{triangles}</triangles></mesh>"#
        )
    }

    const TRIANGLE: &str = r#"<triangle v1="0" v2="1" v3="2"/>"#;

    fn xml(objects: &str) -> String {
        format!(
            r#"<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"><resources>{objects}</resources><build><item objectid="1" transform="1 0 0 0 1 0 0 0 1 0 0 0"/></build></model>"#
        )
    }

    fn normalize_and_sdk(xml: &str) -> Result<(), PreviewError> {
        let (normalized, _) = normalize(xml, &ConversionLimits::default(), Instant::now(), true)?;
        let model = lib3mf::parser::parse_model_xml(normalized.as_deref().unwrap_or(xml))
            .map_err(super::super::classify_three_mf_error)?;
        lib3mf::validator::validate_model(&model).map_err(super::super::classify_three_mf_error)
    }

    #[test]
    fn topology_hint_rejects_non_manifold_at_object_end() {
        let source = xml(&format!(
            r#"<object id="1">{}</object>"#,
            mesh(&TRIANGLE.repeat(3))
        ));
        assert!(matches!(
            normalize(&source, &ConversionLimits::default(), Instant::now(), true),
            Err(PreviewError::NonManifoldMesh)
        ));
    }

    #[test]
    fn topology_hint_keeps_manifold_and_resets_between_objects_and_meshes() {
        let valid = mesh(&TRIANGLE.repeat(2));
        for objects in [
            format!(r#"<object id="1">{valid}</object>"#),
            format!(r#"<object id="1">{valid}</object><object id="2">{valid}</object>"#),
            // The SDK overwrites the first mesh, including its non-manifold edges.
            format!(
                r#"<object id="1">{}{valid}</object>"#,
                mesh(&TRIANGLE.repeat(3))
            ),
        ] {
            assert!(normalize_and_sdk(&xml(&objects)).is_ok());
        }
    }

    #[test]
    fn topology_hint_defers_invalid_indices_and_ambiguous_structure_to_sdk() {
        for triangles in [
            format!(r#"{}<triangle v1="0" v2="1" v3="3"/>"#, TRIANGLE.repeat(3)),
            format!(r#"{}<triangle v1="0" v2="1" v3="-1"/>"#, TRIANGLE.repeat(3)),
            format!(r#"{}<triangle v1="0" v2="1" v3="0"/>"#, TRIANGLE.repeat(3)),
            format!("<unknown>{}</unknown>", TRIANGLE.repeat(3)),
        ] {
            let source = xml(&format!(r#"<object id="1">{}</object>"#, mesh(&triangles)));
            assert!(normalize(&source, &ConversionLimits::default(), Instant::now(), true).is_ok());
            assert!(normalize_and_sdk(&source).is_err());
        }
    }

    #[test]
    fn topology_hint_cap_disables_hint_without_accepting_model() {
        let mut hint = MeshTopologyHint {
            vertices: 3,
            ..MeshTopologyHint::default()
        };
        let mut reader = Reader::from_str(TRIANGLE);
        let Event::Empty(element) = reader.read_event().unwrap() else {
            panic!()
        };
        assert!(hint.triangle(&element, 2).is_none());
        assert!(!hint.non_manifold);
        assert_eq!(hint.edges.len(), 2);
        let source = xml(&format!(
            r#"<object id="1">{}</object>"#,
            mesh(&TRIANGLE.repeat(3))
        ));
        let model = lib3mf::parser::parse_model_xml(&source).unwrap();
        assert!(lib3mf::validator::validate_mesh_geometry(&model).is_err());
    }

    #[test]
    fn topology_hint_does_not_hide_duplicate_attribute_errors() {
        let triangles = format!(
            r#"{}<triangle v1="0" v1="1" v2="1" v3="2"/>"#,
            TRIANGLE.repeat(3)
        );
        let source = xml(&format!(r#"<object id="1">{}</object>"#, mesh(&triangles)));
        assert!(matches!(
            normalize_and_sdk(&source),
            Err(PreviewError::MalformedInput)
        ));
    }
}
