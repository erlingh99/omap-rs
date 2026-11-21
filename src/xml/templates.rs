//! The `<templates>` section: background templates and their transforms.

use crate::templates::{
    AdjustmentState, GdalTemplate, ImageTemplate, MapTemplate, Matrix3x3, OgrTemplate, PassPoint,
    Template, TemplateCommon, TemplateDefaults, TemplateEntry, TemplateTransform,
    TemplateTransformations, Templates, TrackTemplate,
};
use crate::view::TemplateVisibility;
use crate::xml::attrs::{parse_attr_raw, try_get_attr_raw};
use crate::xml::units::{from_file_coords, to_file_coords};
use crate::xml::{Error, Result, Section};
use geo_types::Coord;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::path::PathBuf;

impl TemplateDefaults {
    fn parse_attrs(bs: &BytesStart<'_>) -> Self {
        let mut d = Self::default();
        for attr in bs.attributes().filter_map(std::result::Result::ok) {
            match attr.key.local_name().as_ref() {
                "use_meters_per_pixel" => {
                    d.use_meters_per_pixel = attr.as_bool().unwrap_or(d.use_meters_per_pixel);
                }
                "meters_per_pixel" => {
                    d.meters_per_pixel = parse_attr_raw(attr.value)
                        .unwrap_or_else(|_| d.meters_per_pixel.get())
                        .try_into()
                        .unwrap_or_default();
                }
                "dpi" => {
                    d.dpi = parse_attr_raw(attr.value)
                        .unwrap_or_else(|_| d.dpi.get())
                        .try_into()
                        .unwrap_or_default();
                }
                "scale" => d.scale = parse_attr_raw(attr.value).unwrap_or(d.scale),
                _ => {}
            }
        }
        d
    }

    fn write<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        writer.write_event(Event::Empty(BytesStart::new("defaults").with_attributes([
            (
                "use_meters_per_pixel",
                self.use_meters_per_pixel.to_string().as_str(),
            ),
            (
                "meters_per_pixel",
                format!("{:.2}", self.meters_per_pixel.get()).as_str(),
            ),
            ("dpi", format!("{:.2}", self.dpi.get()).as_str()),
            ("scale", self.scale.to_string().as_str()),
        ])))?;
        Ok(())
    }
}

impl TemplateEntry {
    fn write<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<TemplateVisibility> {
        self.template.write(writer)?;
        Ok(self.visibility)
    }
}

impl Templates {
    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        event: &BytesStart<'_>,
    ) -> Result<Self> {
        let first_front_template = try_get_attr_raw(event, "first_front_template")?.unwrap_or(0);

        let mut templates = Vec::new();
        let mut defaults = TemplateDefaults::default();

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(bs) => match bs.local_name().as_ref() {
                    "template" => {
                        templates.push(Template::parse(reader, &bs)?);
                    }
                    "defaults" => {
                        defaults = TemplateDefaults::parse_attrs(&bs);
                    }
                    _ => {}
                },
                Event::End(be) if be.local_name().as_ref() == "templates" => break,
                Event::Eof => {
                    return Err(Error::UnexpectedEof(Section::Templates));
                }
                _ => {}
            }
        }

        let template_entries = templates
            .into_iter()
            .map(|t| TemplateEntry {
                template: t,
                visibility: TemplateVisibility::default(),
            })
            .collect();

        Ok(Self {
            template_entries,
            first_front_template,
            defaults,
        })
    }

    pub(crate) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<Vec<TemplateVisibility>> {
        writer.write_event(Event::Start(BytesStart::new("templates").with_attributes(
            [
                ("count", self.template_entries.len().to_string().as_str()),
                (
                    "first_front_template",
                    self.first_front_template.to_string().as_str(),
                ),
            ],
        )))?;
        writer.get_mut().write_all(b"\n")?;
        let mut visibilities = Vec::with_capacity(self.len());
        for entry in &self.template_entries {
            visibilities.push(entry.write(writer)?);
            writer.get_mut().write_all(b"\n")?;
        }
        self.defaults.write(writer)?;
        writer.get_mut().write_all(b"\n")?;
        writer.write_event(Event::End(BytesEnd::new("templates")))?;
        Ok(visibilities)
    }
}

impl TrackTemplate {
    fn write<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        if !self.track_crs_spec.is_empty() {
            writer.write_event(Event::Start(BytesStart::new("track_crs_spec")))?;
            writer.write_event(Event::Text(BytesText::new(&self.track_crs_spec)))?;
            writer.write_event(Event::End(BytesEnd::new("track_crs_spec")))?;
        }
        if !self.projected_crs_spec.is_empty() {
            writer.write_event(Event::Start(BytesStart::new("projected_crs_spec")))?;
            writer.write_event(Event::Text(BytesText::new(&self.projected_crs_spec)))?;
            writer.write_event(Event::End(BytesEnd::new("projected_crs_spec")))?;
        }
        Ok(())
    }
}

impl GdalTemplate {
    fn write<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        writer.write_event(Event::Start(BytesStart::new("crs_spec")))?;
        writer.write_event(Event::Text(BytesText::new(&self.crs_spec)))?;
        writer.write_event(Event::End(BytesEnd::new("crs_spec")))?;
        Ok(())
    }
}

impl OgrTemplate {
    fn write<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        if !self.crs_spec.is_empty() {
            writer.write_event(Event::Start(BytesStart::new("crs_spec")))?;
            writer.write_event(Event::Text(BytesText::new(&self.crs_spec)))?;
            writer.write_event(Event::End(BytesEnd::new("crs_spec")))?;
        }
        if !self.track_crs_spec.is_empty() {
            writer.write_event(Event::Start(BytesStart::new("track_crs_spec")))?;
            writer.write_event(Event::Text(BytesText::new(&self.track_crs_spec)))?;
            writer.write_event(Event::End(BytesEnd::new("track_crs_spec")))?;
        }
        if !self.projected_crs_spec.is_empty() {
            writer.write_event(Event::Start(BytesStart::new("projected_crs_spec")))?;
            writer.write_event(Event::Text(BytesText::new(&self.projected_crs_spec)))?;
            writer.write_event(Event::End(BytesEnd::new("projected_crs_spec")))?;
        }
        Ok(())
    }
}

impl TemplateTransformations {
    pub(crate) fn write<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut start = BytesStart::new("transformations");
        match self.adjustment {
            AdjustmentState::Adjusted => start.push_attribute(("adjusted", "true")),
            AdjustmentState::AdjustmentDirty => start.push_attribute(("adjustment_dirty", "true")),
            AdjustmentState::NoAdjustment => {}
        }
        start.push_attribute(("passpoints", self.passpoints.len().to_string().as_str()));

        writer.write_event(Event::Start(start))?;

        self.active_transform.write(writer, "active")?;
        self.other_transform.write(writer, "other")?;

        for pp in &self.passpoints {
            pp.write(writer)?;
        }

        if let Some(map_to_template) = &self.map_to_template {
            map_to_template.write(writer, "map_to_template")?;
        }
        if let Some(template_to_map) = &self.template_to_map {
            template_to_map.write(writer, "template_to_map")?;
        }
        if let Some(template_to_map_other) = &self.template_to_map_other {
            template_to_map_other.write(writer, "template_to_map_other")?;
        }

        writer.write_event(Event::End(BytesEnd::new("transformations")))?;
        Ok(())
    }
}

impl Template {
    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        bs: &BytesStart<'_>,
    ) -> Result<Self> {
        let mut template_type = String::new();
        let mut is_open = false;
        let mut name = String::new();
        let mut path = PathBuf::new();
        let mut relpath = PathBuf::new();
        let mut is_georeferenced = false;
        let mut group = None;

        for attr in bs.attributes().filter_map(std::result::Result::ok) {
            match attr.key.local_name().as_ref() {
                "type" => template_type = parse_attr_raw(attr.value).unwrap_or(template_type),
                "open" => is_open = attr.as_bool().unwrap_or(false),
                "name" => name = parse_attr_raw(attr.value).unwrap_or(name),
                "path" => path = parse_attr_raw(attr.value).unwrap_or(path),
                "relpath" => relpath = parse_attr_raw(attr.value).unwrap_or(relpath),
                "georef" => is_georeferenced = attr.as_bool().unwrap_or(false),
                "group" => group = parse_attr_raw(attr.value).ok(),
                _ => {}
            }
        }

        let mut transformations = None;
        let mut crs_spec = String::new();
        let mut track_crs_spec = String::new();
        let mut projected_crs_spec = String::new();

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(child) => match child.local_name().as_ref() {
                    "transformations" => {
                        transformations = Some(TemplateTransformations::parse(reader, &child)?);
                    }
                    "crs_spec" => {
                        crs_spec = crate::xml::read_text(reader)?;
                    }
                    "projected_crs_spec" => {
                        projected_crs_spec = crate::xml::read_text(reader)?;
                    }
                    "track_crs_spec" => {
                        track_crs_spec = crate::xml::read_text(reader)?;
                    }
                    _ => {}
                },
                Event::End(be) if be.local_name().as_ref() == "template" => break,
                Event::Eof => {
                    return Err(Error::UnexpectedEof(Section::Template));
                }
                _ => {}
            }
        }

        let common = TemplateCommon {
            is_open,
            name,
            path,
            relpath,
            is_georeferenced,
            group,
            transformations,
        };

        let template = match template_type.as_str() {
            "TemplateTrack" => Self::Track(TrackTemplate {
                common,
                track_crs_spec,
                projected_crs_spec,
            }),
            "TemplateMap" => Self::Map(MapTemplate { common }),
            "GdalTemplate" => Self::Gdal(GdalTemplate { common, crs_spec }),
            "OgrTemplate" => Self::Ogr(OgrTemplate {
                common,
                crs_spec,
                track_crs_spec,
                projected_crs_spec,
            }),
            "TemplateImage" => Self::Image(ImageTemplate { common }),
            _ => return Err(Error::TemplateError),
        };
        Ok(template)
    }

    pub(crate) fn write<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let common = self.common();

        let mut start = BytesStart::new("template").with_attributes([
            ("type", self.type_name()),
            ("open", common.is_open.to_string().as_str()),
            ("name", common.name.as_str()),
            ("path", common.path.to_string_lossy().as_ref()),
            ("relpath", common.relpath.to_string_lossy().as_ref()),
        ]);

        if common.is_georeferenced {
            start.push_attribute(("georef", "true"));
        }
        if let Some(group) = common.group {
            start.push_attribute(("group", group.to_string().as_str()));
        }

        writer.write_event(Event::Start(start))?;

        if let Some(t) = &common.transformations {
            t.write(writer)?;
        }

        match &self {
            Self::Gdal(t) => t.write(writer)?,
            Self::Track(t) => t.write(writer)?,
            Self::Ogr(t) => t.write(writer)?,
            Self::Image(_) | Self::Map(_) => {}
        }

        writer.write_event(Event::End(BytesEnd::new("template")))?;
        Ok(())
    }
}

impl TemplateTransformations {
    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        bs: &BytesStart<'_>,
    ) -> Result<Self> {
        let mut adjustment = AdjustmentState::NoAdjustment;

        for attr in bs.attributes().filter_map(std::result::Result::ok) {
            match attr.key.local_name().as_ref() {
                "adjusted" if attr.as_bool().unwrap_or(false) => {
                    adjustment = AdjustmentState::Adjusted;
                }
                "adjustment_dirty" if attr.as_bool().unwrap_or(false) => {
                    adjustment = AdjustmentState::AdjustmentDirty;
                }
                _ => {}
            }
        }

        let mut active_transform = TemplateTransform::default();
        let mut other_transform = TemplateTransform::default();
        let mut passpoints = Vec::new();
        let mut map_to_template = None;
        let mut template_to_map = None;
        let mut template_to_map_other = None;

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(child) => match child.local_name().as_ref() {
                    "transformation" => {
                        let t = TemplateTransform::parse(&child);
                        if let Some(role) = child
                            .attributes()
                            .filter_map(std::result::Result::ok)
                            .find(|a| a.key.local_name().as_ref() == "role")
                        {
                            match role.value.as_ref() {
                                "other" => other_transform = t,
                                _ => active_transform = t,
                            }
                        }
                    }
                    "passpoint" => {
                        passpoints.push(PassPoint::parse(reader)?);
                    }
                    "matrix" => {
                        let m = Matrix3x3::parse(reader)?;
                        if let Some(role) = child
                            .attributes()
                            .filter_map(std::result::Result::ok)
                            .find(|a| a.key.local_name().as_ref() == "role")
                        {
                            match role.value.as_ref() {
                                "map_to_template" => map_to_template = Some(m),
                                "template_to_map" => template_to_map = Some(m),
                                "template_to_map_other" => template_to_map_other = Some(m),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                },
                Event::End(be) if be.local_name().as_ref() == "transformations" => {
                    break;
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(Section::TemplateTransformations));
                }
                _ => {}
            }
        }

        Ok(Self {
            adjustment,
            active_transform,
            other_transform,
            passpoints,
            map_to_template,
            template_to_map,
            template_to_map_other,
        })
    }
}

impl TemplateTransform {
    pub(crate) fn parse(bs: &BytesStart<'_>) -> Self {
        let mut t = Self::default();
        let mut pos = Coord::default();
        for attr in bs.attributes().filter_map(std::result::Result::ok) {
            match attr.key.local_name().as_ref() {
                "x" => pos.x = parse_attr_raw::<i32>(attr.value).unwrap_or(0),
                "y" => pos.y = parse_attr_raw::<i32>(attr.value).unwrap_or(0),
                "rotation" => t.template_rotation = parse_attr_raw(attr.value).unwrap_or(0.),
                "scale_x" => t.template_scale.x = parse_attr_raw(attr.value).unwrap_or(1.),
                "scale_y" => t.template_scale.y = parse_attr_raw(attr.value).unwrap_or(1.),
                "shear" => t.template_shear = parse_attr_raw(attr.value).unwrap_or(0.),
                _ => {}
            }
        }
        t.template_pos = from_file_coords(pos);
        t
    }

    pub(crate) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        role: &str,
    ) -> Result<()> {
        let pos = to_file_coords(self.template_pos)?;
        let mut start = BytesStart::new("transformation").with_attributes([
            ("role", role),
            ("x", pos.x.to_string().as_str()),
            ("y", pos.y.to_string().as_str()),
            ("scale_x", self.template_scale.x.to_string().as_str()),
            ("scale_y", self.template_scale.y.to_string().as_str()),
            ("rotation", self.template_rotation.to_string().as_str()),
        ]);
        if self.template_shear != 0.0 {
            start.push_attribute(("shear", self.template_shear.to_string().as_str()));
        }
        writer.write_event(Event::Empty(start))?;
        Ok(())
    }
}

impl PassPoint {
    pub(crate) fn parse<R: std::io::BufRead>(reader: &mut Reader<R>) -> Result<Self> {
        let mut src = None;
        let mut dest = None;
        let mut calc = None;

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(child) => match child.local_name().as_ref() {
                    "source" => src = parse_inner_coord(reader).ok(),
                    "destination" => dest = parse_inner_coord(reader).ok(),
                    "calculated" => calc = parse_inner_coord(reader).ok(),
                    _ => {}
                },
                Event::End(be) if be.local_name().as_ref() == "passpoint" => {
                    break;
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(Section::PassPoint));
                }
                _ => {}
            }
        }

        if let Some(src_coord) = src
            && let Some(dest_coord) = dest
            && let Some(calculated_coord) = calc
        {
            Ok(Self {
                src_coord,
                dest_coord,
                calculated_coord,
            })
        } else {
            Err(Error::TemplateError)
        }
    }

    pub(crate) fn write<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        writer
            .write_event(Event::Start(BytesStart::new("passpoint").with_attributes(
                [("error", format!("{:.5}", self.error()).as_str())],
            )))?;
        write_coord_wrapper(writer, "source", &self.src_coord)?;
        write_coord_wrapper(writer, "destination", &self.dest_coord)?;
        write_coord_wrapper(writer, "calculated", &self.calculated_coord)?;

        writer.write_event(Event::End(BytesEnd::new("passpoint")))?;
        Ok(())
    }
}

impl Matrix3x3 {
    pub(crate) fn parse<R: std::io::BufRead>(reader: &mut Reader<R>) -> Result<Self> {
        let mut values = [0.; 9];
        let mut i = 0;
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(child) if child.local_name().as_ref() == "element" => {
                    values[i] = try_get_attr_raw(&child, "value")?.unwrap_or(0.);
                    i += 1;
                }
                Event::End(be) if be.local_name().as_ref() == "matrix" => break,
                Event::Eof => {
                    return Err(Error::UnexpectedEof(Section::Matrix));
                }
                _ => {}
            }
        }

        Ok(Self(values))
    }

    pub(crate) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        role: &str,
    ) -> Result<()> {
        writer.write_event(Event::Start(BytesStart::new("matrix").with_attributes([
            ("role", role),
            ("n", "3"),
            ("m", "3"),
        ])))?;
        for val in self.0 {
            writer.write_event(Event::Empty(
                BytesStart::new("element").with_attributes([("value", format!("{val}").as_str())]),
            ))?;
        }
        writer.write_event(Event::End(BytesEnd::new("matrix")))?;
        Ok(())
    }
}

fn write_coord_wrapper<W: std::io::Write>(
    writer: &mut Writer<W>,
    wrapper_name: &str,
    coord: &Coord,
) -> Result<()> {
    let coord = to_file_coords(*coord)?;
    writer.write_event(Event::Start(BytesStart::new(wrapper_name)))?;
    writer.write_event(Event::Empty(BytesStart::new("coord").with_attributes([
        ("x", format!("{}", coord.x).as_str()),
        ("y", format!("{}", coord.y).as_str()),
    ])))?;
    writer.write_event(Event::End(BytesEnd::new(wrapper_name)))?;
    Ok(())
}

fn parse_inner_coord<R: std::io::BufRead>(reader: &mut Reader<R>) -> Result<Coord> {
    let mut coord = Coord::<i32>::zero();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(bs) if bs.local_name().as_ref() == "coord" => {
                coord = Coord {
                    x: try_get_attr_raw(&bs, "x")?.unwrap_or(0),
                    y: try_get_attr_raw(&bs, "y")?.unwrap_or(0),
                };
            }
            Event::End(_) => break,
            Event::Eof => {
                return Err(Error::UnexpectedEof(Section::CoordinateWrapper));
            }
            _ => {}
        }
    }
    Ok(from_file_coords(coord))
}
