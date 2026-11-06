//! Rejection-only count hint. Every candidate still passes the full SDK validator.
use std::{collections::HashMap, time::Instant};

use quick_xml::{NsReader, events::Event, name::ResolveResult};

use super::{ConversionLimits, ThreeMfModel, check_timeout, three_mf_part_path};

const ROOT: &str = "3D/3dmodel.model";
const CORE: &str = "http://schemas.microsoft.com/3dmanufacturing/core/2015/02";
const PRODUCTION: &str = "http://schemas.microsoft.com/3dmanufacturing/production/2015/06";
const MAX_METADATA: usize = 4096;
type Key = (String, usize);

#[derive(Default)]
struct Object {
    triangles: usize,
    children: Vec<Key>,
}

pub(super) struct Budget {
    roots: Vec<Key>,
    parts: HashMap<String, HashMap<usize, Object>>,
    remaining: usize,
}

impl Budget {
    pub(super) fn new(root: ThreeMfModel) -> Option<Self> {
        let roots = if root.build.items.is_empty() {
            root.resources
                .objects
                .iter()
                .map(|o| (ROOT.to_owned(), o.id))
                .collect::<Vec<_>>()
        } else {
            root.build
                .items
                .iter()
                .map(|item| {
                    Some((
                        three_mf_part_path(item.production_path.as_deref(), ROOT)
                            .ok()?
                            .to_owned(),
                        item.objectid,
                    ))
                })
                .collect::<Option<Vec<_>>>()?
        };
        (roots.len() <= MAX_METADATA).then_some(Self {
            roots,
            parts: HashMap::new(),
            remaining: MAX_METADATA,
        })
    }

    pub(super) fn add(
        &mut self,
        path: &str,
        xml: &str,
        limits: &ConversionLimits,
        started: Instant,
    ) -> Option<()> {
        let objects = summary(path, xml, limits, started, &mut self.remaining)?;
        self.parts
            .insert(path.to_owned(), objects)
            .is_none()
            .then_some(())
    }

    pub(super) fn exceeds(&self, limit: usize) -> Option<bool> {
        fn count(
            budget: &Budget,
            key: &Key,
            limit: usize,
            stack: &mut Vec<Key>,
            memo: &mut HashMap<Key, usize>,
        ) -> Option<usize> {
            if stack.len() >= 64 || stack.contains(key) {
                return None;
            }
            if let Some(&value) = memo.get(key) {
                return Some(value);
            }
            let object = budget.parts.get(&key.0)?.get(&key.1)?;
            stack.push(key.clone());
            let mut triangles = object.triangles;
            for child in &object.children {
                triangles = triangles
                    .saturating_add(count(budget, child, limit, stack, memo)?)
                    .min(limit.saturating_add(1));
            }
            stack.pop();
            memo.insert(key.clone(), triangles);
            Some(triangles)
        }
        // Pass the same memo across roots: instances add their counts each time,
        // while shared object definitions are scanned only once.
        let mut total = 0usize;
        let mut memo = HashMap::new();
        for root in &self.roots {
            total = total
                .saturating_add(count(self, root, limit, &mut Vec::new(), &mut memo)?)
                .min(limit.saturating_add(1));
        }
        Some(total > limit)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tag {
    Model,
    Resources,
    Object,
    Mesh,
    Triangles,
    Triangle,
    Components,
    Component,
    Other,
}

fn summary(
    path: &str,
    xml: &str,
    limits: &ConversionLimits,
    started: Instant,
    remaining: &mut usize,
) -> Option<HashMap<usize, Object>> {
    let mut reader = NsReader::from_str(xml);
    let mut stack = Vec::new();
    let mut objects = HashMap::<usize, Object>::new();
    let mut object_id = None;
    let mut model = false;
    let mut events = 0usize;
    loop {
        if events.is_multiple_of(1024) {
            check_timeout(started, limits).ok()?;
        }
        events += 1;
        let (namespace, event) = reader.read_resolved_event().ok()?;
        let core = matches!(namespace, ResolveResult::Bound(ns) if ns.0 == CORE);
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let tag = match (core, element.local_name().as_ref()) {
                    (true, "model") => Tag::Model,
                    (true, "resources") => Tag::Resources,
                    (true, "object") => Tag::Object,
                    (true, "mesh") => Tag::Mesh,
                    (true, "triangles") => Tag::Triangles,
                    (true, "triangle") => Tag::Triangle,
                    (true, "components") => Tag::Components,
                    (true, "component") => Tag::Component,
                    _ => Tag::Other,
                };
                if stack.is_empty() {
                    if model || tag != Tag::Model {
                        return None;
                    }
                    model = true;
                }
                let at_object = stack == [Tag::Model, Tag::Resources] && tag == Tag::Object;
                let at_component = stack
                    == [Tag::Model, Tag::Resources, Tag::Object, Tag::Components]
                    && tag == Tag::Component;
                let mut id = None;
                let mut child_path = None;
                if at_object || at_component {
                    *remaining = remaining.checked_sub(1)?;
                    for attr in element.attributes() {
                        let attr = attr.ok()?;
                        let (namespace, local) = reader.resolver().resolve_attribute(attr.key);
                        let value = || {
                            attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .ok()
                        };
                        if namespace == ResolveResult::Unbound
                            && local.as_ref() == if at_object { "id" } else { "objectid" }
                        {
                            if id.is_some() {
                                return None;
                            }
                            id = Some(value()?.parse::<usize>().ok()?);
                        } else if local.as_ref() == "path" {
                            // The SDK reads path by local name. Ambiguous/non-production
                            // attributes must not make this hint follow a different graph.
                            if !matches!(namespace, ResolveResult::Bound(ns) if ns.0 == PRODUCTION)
                            {
                                return None;
                            }
                            let value = value()?;
                            if child_path.is_some() || value.len() > 1024 {
                                return None;
                            }
                            child_path = Some(value.into_owned());
                        }
                    }
                }
                if at_object {
                    let id = id?;
                    if objects.insert(id, Object::default()).is_some() {
                        return None;
                    }
                    object_id = (!empty).then_some(id);
                } else if at_component {
                    let child = (
                        three_mf_part_path(child_path.as_deref(), path)
                            .ok()?
                            .to_owned(),
                        id?,
                    );
                    objects.get_mut(&object_id?)?.children.push(child);
                } else if tag == Tag::Triangle
                    && stack
                        == [
                            Tag::Model,
                            Tag::Resources,
                            Tag::Object,
                            Tag::Mesh,
                            Tag::Triangles,
                        ]
                {
                    let object = objects.get_mut(&object_id?)?;
                    object.triangles = object.triangles.saturating_add(1);
                }
                if !empty {
                    if stack.len() >= 64 {
                        return None;
                    }
                    stack.push(tag);
                }
            }
            Event::End(_) if stack.pop()? == Tag::Object => object_id = None,
            Event::DocType(_) => return None,
            // Escaped text cannot change object IDs, paths or triangle counts.
            // Custom entities still disable the hint; the full SDK remains authoritative.
            Event::GeneralRef(reference)
                if !matches!(reference.as_ref(), "amp" | "lt" | "gt" | "quot" | "apos")
                    && reference.resolve_char_ref().ok()?.is_none() =>
            {
                return None;
            }
            Event::Eof => return (model && stack.is_empty()).then_some(objects),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xml(objects: &str, build: &str) -> String {
        format!(
            r#"<model xmlns="{}" xmlns:p="{}" unit="millimeter"><resources>{objects}</resources><build>{build}</build></model>"#,
            CORE, PRODUCTION
        )
    }
    fn mesh(id: usize, triangles: usize) -> String {
        format!(
            r#"<object id="{id}" type="model"><mesh><vertices><vertex x="0" y="0" z="0"/></vertices><triangles>{}</triangles></mesh></object>"#,
            r#"<triangle v1="0" v2="0" v3="0"/>"#.repeat(triangles)
        )
    }
    fn budget(root: &str) -> Budget {
        let model = lib3mf::parser::parse_model_from_reader(
            std::io::Cursor::new(root.as_bytes()),
            lib3mf::ParserConfig::with_all_extensions()
                .with_spec_conformance(lib3mf::SpecConformance::Lenient),
        )
        .unwrap();
        let mut result = Budget::new(model).unwrap();
        result
            .add(ROOT, root, &ConversionLimits::default(), Instant::now())
            .unwrap();
        result
    }
    fn add(budget: &mut Budget, name: &str, xml: &str) -> Option<()> {
        budget.add(name, xml, &ConversionLimits::default(), Instant::now())
    }

    #[test]
    fn repeated_assembly_counts_instances_and_ignores_unused_meshes() {
        let root = xml(
            r#"<object id="2" type="model"><components><component objectid="1" p:path="/3D/part.model"/><component objectid="1" p:path="/3D/part.model"/></components></object>"#,
            r#"<item objectid="2"/><item objectid="2"/>"#,
        );
        let mut value = budget(&root);
        add(
            &mut value,
            "3D/part.model",
            &xml(&(mesh(1, 3) + &mesh(9, 100)), ""),
        )
        .unwrap();
        assert_eq!(value.exceeds(11), Some(true));
        assert_eq!(value.exceeds(12), Some(false));
        assert_eq!(value.exceeds(100), Some(false));
        let empty_build = xml(&(mesh(1, 2) + &mesh(2, 3)), "");
        assert_eq!(budget(&empty_build).exceeds(4), Some(true));
        assert_eq!(budget(&empty_build).exceeds(5), Some(false));
    }

    #[test]
    fn missing_cyclic_ambiguous_or_oversized_metadata_provides_no_hint() {
        let root = xml(
            r#"<object id="1" type="model"><components><component objectid="2" p:path="/3D/part.model"/></components></object>"#,
            r#"<item objectid="1"/>"#,
        );
        let mut value = budget(&root);
        assert_eq!(value.exceeds(0), None);
        add(&mut value, "3D/part.model", &xml(r#"<object id="2" type="model"><components><component objectid="1" p:path="/3D/3dmodel.model"/></components></object>"#, "")).unwrap();
        assert_eq!(value.exceeds(0), None);
        for part in [
            xml(
                r#"<object id="2" type="model"><components><component objectid="1" path="/3D/part.model"/></components></object>"#,
                "",
            ),
            xml(&(mesh(2, 1) + &mesh(2, 1)), ""),
            xml(&mesh(2, 1), "").replace(CORE, "urn:other"),
            xml(&mesh(2, 1), "").replace("</model>", ""),
        ] {
            assert!(add(&mut budget(&root), "3D/part.model", &part).is_none());
        }
        let mut value = budget(&root);
        value.remaining = 0;
        assert!(add(&mut value, "3D/part.model", &xml(&mesh(2, 1), "")).is_none());
    }

    #[test]
    fn escaped_metadata_does_not_disable_the_count_hint() {
        let root = xml(
            r#"<object id="1" type="model"><components><component objectid="2" p:path="/3D/part.model"/></components></object>"#,
            r#"<item objectid="1"/>"#,
        ).replace("<resources>", r#"<metadata name="Application">A &amp; B &lt; &gt; &quot; &apos; &#38; &#x26;</metadata><resources>"#);
        let part = xml(&mesh(2, 3), "").replace(
            "<resources>",
            r#"<metadata name="Application">A &amp; B</metadata><resources>"#,
        );
        let mut value = budget(&root);
        add(&mut value, "3D/part.model", &part).unwrap();
        assert_eq!(value.exceeds(2), Some(true));
        assert_eq!(value.exceeds(3), Some(false));
        for unknown in ["&unresolved;", "&#xZZ;"] {
            assert!(
                add(
                    &mut budget(&root),
                    "3D/part.model",
                    &part.replace("&amp;", unknown)
                )
                .is_none()
            );
        }
    }

    #[test]
    fn namespace_aliases_and_saturating_instance_counts_are_bounded() {
        let root = xml(
            r#"<object id="1" type="model"><components><component objectid="2" p:path="/3D/part.model"/><component objectid="2" p:path="/3D/part.model"/></components></object>"#,
            r#"<item objectid="1"/>"#,
        );
        let root = root
            .replace("xmlns:p", "xmlns:q")
            .replace("p:path", "q:path");
        let mut value = budget(&root);
        let part = xml(&mesh(2, 1), "")
            .replace("<model ", "<c:model ")
            .replace("</model>", "</c:model>");
        // Explicitly prefixed root with a core default namespace still resolves safely.
        let part = part.replace("<c:model ", &format!("<c:model xmlns:c=\"{CORE}\" "));
        add(&mut value, "3D/part.model", &part).unwrap();
        assert_eq!(value.exceeds(1), Some(true));
        let object = value
            .parts
            .get_mut("3D/part.model")
            .unwrap()
            .get_mut(&2)
            .unwrap();
        object.triangles = usize::MAX;
        assert_eq!(value.exceeds(10), Some(true));
    }
}
