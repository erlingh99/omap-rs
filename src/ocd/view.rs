//! The map notes (type 1061) and the view (type 1030): its center, zoom and
//! display modes.

use geo_types::Coord;

use crate::NonNegativeF64;
use crate::ocd::strings::{ParameterString, kind, number};
use crate::view::{GridUnit, View};

/// The map notes, with OCAD's CR LF line breaks as LF.
pub(in crate::ocd) fn read_notes(strings: &[ParameterString]) -> String {
    strings
        .iter()
        .filter(|string| string.kind == kind::NOTES)
        .map(|string| string.text.replace("\r\n", "\n"))
        .collect()
}

/// The view, with the grid spacing in metres the georeferencing states.
pub(in crate::ocd) fn read_view(strings: &[ParameterString], grid_spacing: Option<f64>) -> View {
    let mut view = View::default();
    if let Some(spacing) = grid_spacing {
        view.grid.unit = GridUnit::MetersOnGround;
        view.grid.h_spacing = spacing;
        view.grid.v_spacing = spacing;
    }
    let Some(string) = strings.iter().find(|string| string.kind == kind::VIEW) else {
        return view;
    };
    let mut centre = Coord::zero();
    for (field, value) in string.fields() {
        let Some(value) = number(value) else {
            continue;
        };
        match field {
            'x' => centre.x = value,
            'y' => centre.y = value,
            'z' if value > 0. => view.zoom = NonNegativeF64::clamped_from(value),
            'h' => view.area_hatching_enabled = value.round() as i32 != 0,
            'k' => view.baseline_view_enabled = value.round() as i32 != 0,
            _ => (),
        }
    }
    view.view_centre = centre;
    view
}

/// The notes string, if there are notes.
pub(in crate::ocd) fn write_notes(notes: &str) -> Option<ParameterString> {
    (!notes.is_empty()).then(|| ParameterString::new(kind::NOTES, &notes.replace('\n', "\r\n")))
}

/// The view string, with the fields Mapper writes.
pub(in crate::ocd) fn write_view(view: &View) -> ParameterString {
    let mut string = ParameterString::new(kind::VIEW, "");
    string.push('x', view.view_centre.x);
    string.push('y', view.view_centre.y);
    string.push('z', view.zoom.get());
    string.push('v', 0);
    string.push('m', 50);
    string.push('t', 50);
    string.push('b', 50);
    string.push('c', 50);
    string.push('h', i32::from(view.area_hatching_enabled));
    string.push('d', 0);
    string.push('k', i32::from(view.baseline_view_enabled));
    string
}
