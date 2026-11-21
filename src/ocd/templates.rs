//! Background maps: type 8 parameter strings, read as templates.
//!
//! Mapper adds each background map below the ones before it, so the last in
//! the file is drawn first; reading keeps that order. All are drawn behind
//! the map.

use std::path::{Path, PathBuf};

use crate::UnitF64;
use crate::ocd::strings::{ParameterString, kind, number};
use crate::ocd::warning::{Warning, Warnings, push_once};
use crate::templates::{
    AdjustmentState, ImageTemplate, MapTemplate, Template, TemplateCommon, TemplateEntry,
    TemplateTransform, TemplateTransformations, Templates, TrackTemplate,
};
use crate::view::TemplateVisibility;

/// Read the background maps.
pub(in crate::ocd) fn read_templates(
    strings: &[ParameterString],
    warnings: &mut Warnings,
) -> Templates {
    let mut entries: Vec<TemplateEntry> = strings
        .iter()
        .filter(|string| string.kind == kind::TEMPLATE)
        .map(|string| read_template(string, warnings))
        .collect();
    entries.reverse();
    Templates {
        first_front_template: u32::try_from(entries.len()).unwrap_or(u32::MAX),
        template_entries: entries,
        defaults: Default::default(),
    }
}

fn read_template(string: &ParameterString, warnings: &mut Warnings) -> TemplateEntry {
    let path = PathBuf::from(string.first().replace('\\', "/"));
    let mut transform = TemplateTransform::default();
    let mut positioned = false;
    let mut angles = Vec::new();
    let mut dimming = 0.;
    let mut visible = false;
    for (field, value) in string.fields() {
        let Some(value) = number(value) else {
            continue;
        };
        match field {
            'x' => {
                positioned = true;
                transform.template_pos.x = value;
            }
            'y' => {
                positioned = true;
                transform.template_pos.y = value;
            }
            'a' | 'b' => {
                positioned = true;
                angles.push(value);
            }
            'u' if value.abs() >= 1e-10 => {
                positioned = true;
                transform.template_scale.x = value;
            }
            'v' if value.abs() >= 1e-10 => {
                positioned = true;
                transform.template_scale.y = value;
            }
            'd' => dimming = value,
            's' => visible = value.round() as i32 != 0,
            _ => (),
        }
    }
    if let [first, rest @ ..] = angles.as_slice() {
        if rest.iter().any(|angle| (angle - first).abs() > 1e-9) {
            push_once(
                warnings,
                Warning::Map("a background map's two angles differ; their average is used"),
            );
        }
        let average = angles.iter().sum::<f64>() / angles.len() as f64;
        transform.template_rotation = average.to_radians();
    }

    let common = TemplateCommon {
        is_open: false,
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        relpath: path.clone(),
        path: path.clone(),
        is_georeferenced: false,
        group: None,
        transformations: positioned.then(|| TemplateTransformations {
            adjustment: AdjustmentState::NoAdjustment,
            active_transform: transform,
            other_transform: TemplateTransform::default(),
            passpoints: Vec::new(),
            map_to_template: None,
            template_to_map: None,
            template_to_map_other: None,
        }),
    };
    TemplateEntry {
        template: template_of_kind(&path, common),
        visibility: TemplateVisibility {
            opacity: UnitF64::clamped_from((100. - dimming) / 100.),
            visible,
        },
    }
}

/// The kind of template a file of this name is, by its extension.
fn template_of_kind(path: &Path, common: TemplateCommon) -> Template {
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase());
    match extension.as_deref() {
        Some("ocd" | "omap" | "xmap") => Template::Map(MapTemplate { common }),
        Some("gpx") => Template::Track(TrackTemplate {
            common,
            track_crs_spec: String::new(),
            projected_crs_spec: String::new(),
        }),
        _ => Template::Image(ImageTemplate { common }),
    }
}

/// The background map strings for `templates`, in the order reading gives
/// back.
pub(in crate::ocd) fn write_templates(
    templates: &Templates,
    warnings: &mut Warnings,
) -> Vec<ParameterString> {
    if usize::try_from(templates.first_front_template).is_ok_and(|front| front < templates.len()) {
        push_once(
            warnings,
            Warning::Map(
                "OCAD draws every background map behind the map; templates in front were moved behind",
            ),
        );
    }
    templates
        .template_entries
        .iter()
        .rev()
        .map(|entry| write_template(entry, warnings))
        .collect()
}

fn write_template(entry: &TemplateEntry, warnings: &mut Warnings) -> ParameterString {
    let common = entry.template.common();
    if common.is_georeferenced {
        push_once(
            warnings,
            Warning::Map("a georeferenced template is placed by the map's georeferencing in OCAD"),
        );
    }
    let mut string = ParameterString::new(kind::TEMPLATE, &common.path.to_string_lossy());
    string.push('s', i32::from(entry.visibility.visible));
    string.push('r', 1);
    if let Some(transformations) = &common.transformations {
        let transform = &transformations.active_transform;
        if transform.template_shear != 0. || !transformations.passpoints.is_empty() {
            push_once(
                warnings,
                Warning::Map("a template's shear and pass points are not supported"),
            );
        }
        let degrees = transform.template_rotation.to_degrees();
        string.push('u', transform.template_scale.x);
        string.push('v', transform.template_scale.y);
        string.push('x', transform.template_pos.x);
        string.push('y', transform.template_pos.y);
        string.push('a', degrees);
        string.push('b', degrees);
    }
    let dimming = ((1. - entry.visibility.opacity.get()) * 100.).round();
    string.push('d', dimming);
    string
}

#[cfg(test)]
mod tests {
    use geo_types::Coord;

    use super::read_templates;
    use crate::ocd::strings::{ParameterString, kind};
    use crate::templates::Template;
    use crate::templates::TemplateEntry;

    /// The position a template is at.
    fn position(entry: &TemplateEntry) -> Option<Coord> {
        entry
            .template
            .common()
            .active_transform()
            .map(|transform| transform.template_pos)
    }

    fn string(text: &str) -> ParameterString {
        ParameterString {
            kind: kind::TEMPLATE,
            text: text.to_owned(),
        }
    }

    #[test]
    fn background_maps_are_placed_scaled_rotated_and_dimmed() {
        let mut warnings = Vec::new();
        let templates = read_templates(
            &[
                string("C:\\maps\\base.ocd\ts1"),
                string("photo.jpg\tx12.5\ty-3\ta10\tb10\tu0.5\tv0.25\td40\ts1"),
            ],
            &mut warnings,
        );
        assert!(warnings.is_empty());
        assert_eq!(templates.len(), 2);
        assert_eq!(templates.first_front_template, 2);

        let photo = &templates.template_entries[0];
        assert!(matches!(photo.template, Template::Image(_)));
        assert_eq!(photo.template.common().name, "photo.jpg");
        assert_eq!(position(photo), Some(Coord { x: 12.5, y: -3. }));
        let transform = photo.template.common().active_transform();
        assert!(
            transform.is_some_and(|t| (t.template_rotation - 10_f64.to_radians()).abs() < 1e-12)
        );
        assert!(transform.is_some_and(|t| t.template_scale == Coord { x: 0.5, y: 0.25 }));
        assert!((photo.visibility.opacity.get() - 0.6).abs() < 1e-12);
        assert!(photo.visibility.visible);

        let base = &templates.template_entries[1];
        assert!(matches!(base.template, Template::Map(_)));
        assert_eq!(
            base.template.common().path.to_string_lossy(),
            "C:/maps/base.ocd"
        );
        assert_eq!(
            position(base),
            None,
            "an unpositioned map takes the map's own georeferencing"
        );
    }
}
