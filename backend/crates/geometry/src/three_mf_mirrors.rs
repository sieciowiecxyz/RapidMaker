//! Normalize mirrors only in the private package supplied to lib3mf, then restore
//! the exact transforms after package validation. Secondary parts retain their
//! existing XML parser path. No source is changed.
use super::{BAMBU_TRIANGLE_SETTINGS, ConversionLimits, PreviewError, ThreeMfModel, check_timeout};
use quick_xml::{Reader, Writer, events::Event};
use std::collections::HashMap;
use std::time::Instant;

type Transform = [f64; 12];
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
