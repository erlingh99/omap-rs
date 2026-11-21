//! The `<view>` section: zoom, grid and template visibility.

use crate::NonNegativeF64;
use crate::templates::Templates;
use crate::utils::UnitF64;
use crate::view::{Grid, GridAlignment, GridDisplay, GridUnit, TemplateVisibility, View};
use crate::xml::attrs::{parse_attr_raw, try_get_attr_raw};
use crate::xml::{Error, Result};
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};
use std::str::FromStr;

impl TemplateVisibility {
    fn parse_map_attrs(bs: &BytesStart<'_>) -> Self {
        let mut tv = Self::default();
        for attr in bs.attributes().filter_map(std::result::Result::ok) {
            match attr.key.local_name().as_ref() {
                "opacity" => {
                    tv.opacity = UnitF64::clamped_from(
                        parse_attr_raw(attr.value).unwrap_or_else(|_| tv.opacity.get()),
                    );
                }
                "visible" => tv.visible = attr.as_bool().unwrap_or(tv.visible),
                _ => (),
            }
        }
        tv
    }
}

impl FromStr for GridDisplay {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "0" => Ok(Self::Hidden),
            "1" => Ok(Self::AllLines),
            "2" => Ok(Self::HorizontalLines),
            "3" => Ok(Self::VerticalLines),
            _ => Err(Error::ViewError),
        }
    }
}

impl From<u8> for GridDisplay {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::AllLines,
            2 => Self::HorizontalLines,
            3 => Self::VerticalLines,
            _ => Self::Hidden,
        }
    }
}

impl AsRef<str> for GridDisplay {
    fn as_ref(&self) -> &str {
        match self {
            Self::Hidden => "0",
            Self::AllLines => "1",
            Self::HorizontalLines => "2",
            Self::VerticalLines => "3",
        }
    }
}

impl FromStr for GridAlignment {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "0" => Ok(Self::MagneticNorth),
            "1" => Ok(Self::GridNorth),
            "2" => Ok(Self::TrueNorth),
            _ => Err(Error::ViewError),
        }
    }
}

impl From<u8> for GridAlignment {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::GridNorth,
            2 => Self::TrueNorth,
            _ => Self::MagneticNorth,
        }
    }
}

impl AsRef<str> for GridAlignment {
    fn as_ref(&self) -> &str {
        match self {
            Self::MagneticNorth => "0",
            Self::GridNorth => "1",
            Self::TrueNorth => "2",
        }
    }
}

impl FromStr for GridUnit {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "0" => Ok(Self::MetersOnGround),
            "1" => Ok(Self::MillimetresOnMap),
            "2" => Ok(Self::PixelsOnScreen),
            _ => Err(Error::ViewError),
        }
    }
}

impl From<u8> for GridUnit {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::MillimetresOnMap,
            2 => Self::PixelsOnScreen,
            _ => Self::MetersOnGround,
        }
    }
}

impl AsRef<str> for GridUnit {
    fn as_ref(&self) -> &str {
        match self {
            Self::MetersOnGround => "0",
            Self::MillimetresOnMap => "1",
            Self::PixelsOnScreen => "2",
        }
    }
}

impl Grid {
    fn parse_attrs(bs: &BytesStart<'_>) -> Self {
        let mut g = Self::default();
        for attr in bs.attributes().filter_map(std::result::Result::ok) {
            match attr.key.local_name().as_ref() {
                "color" => g.color = parse_attr_raw(attr.value).unwrap_or(g.color),
                "display" => g.display = parse_attr_raw(attr.value).unwrap_or(g.display),
                "alignment" => g.alignment = parse_attr_raw(attr.value).unwrap_or(g.alignment),
                "unit" => g.unit = parse_attr_raw(attr.value).unwrap_or(g.unit),
                "additional_rotation" => {
                    g.additional_rotation =
                        parse_attr_raw(attr.value).unwrap_or(g.additional_rotation);
                }
                "h_spacing" => g.h_spacing = parse_attr_raw(attr.value).unwrap_or(g.h_spacing),
                "v_spacing" => g.v_spacing = parse_attr_raw(attr.value).unwrap_or(g.v_spacing),
                "h_offset" => g.h_offset = parse_attr_raw(attr.value).unwrap_or(g.h_offset),
                "v_offset" => g.v_offset = parse_attr_raw(attr.value).unwrap_or(g.v_offset),
                "snapping_enabled" => {
                    g.snapping_enabled = attr.as_bool().unwrap_or(g.snapping_enabled);
                }
                _ => (),
            }
        }
        g
    }

    fn write<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        writer.write_event(Event::Empty(BytesStart::new("grid").with_attributes([
            ("color", self.color.to_string().as_str()),
            ("display", self.display.as_ref()),
            ("alignment", self.alignment.as_ref()),
            (
                "additional_rotation",
                self.additional_rotation.to_string().as_str(),
            ),
            ("unit", self.unit.as_ref()),
            ("h_spacing", self.h_spacing.to_string().as_str()),
            ("v_spacing", self.v_spacing.to_string().as_str()),
            ("h_offset", self.h_offset.to_string().as_str()),
            ("v_offset", self.v_offset.to_string().as_str()),
            (
                "snapping_enabled",
                self.snapping_enabled.to_string().as_str(),
            ),
        ])))?;
        Ok(())
    }
}

impl View {
    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        bs: &BytesStart<'_>,
        templates: &mut Templates,
    ) -> Result<Self> {
        let mut view = Self::default();
        let mut buf = Vec::new();

        view.area_hatching_enabled = try_get_attr_raw(bs, "area_hatching_enabled")
            .ok()
            .flatten()
            .unwrap_or(false);
        view.baseline_view_enabled = try_get_attr_raw(bs, "baseline_view_enabled")
            .ok()
            .flatten()
            .unwrap_or(false);

        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(bs) => match bs.local_name().as_ref() {
                    "grid" => view.grid = Grid::parse_attrs(&bs),
                    "map_view" => view.parse_map_view(reader, &bs, templates)?,
                    _ => {}
                },
                Event::End(be) if be.local_name().as_ref() == "view" => break,
                Event::Eof => break,
                _ => {}
            }
        }

        Ok(view)
    }

    fn parse_map_view<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        bs: &BytesStart<'_>,
        templates: &mut Templates,
    ) -> Result<()> {
        self.zoom = NonNegativeF64::clamped_from(
            try_get_attr_raw(bs, "zoom").ok().flatten().unwrap_or(1.0),
        );
        self.rotation = try_get_attr_raw(bs, "rotation")
            .ok()
            .flatten()
            .unwrap_or(0.0);
        self.view_centre.x = crate::xml::units::from_file_value(
            try_get_attr_raw(bs, "position_x")
                .ok()
                .flatten()
                .unwrap_or(0),
        );
        self.view_centre.y = crate::xml::units::from_file_value(
            try_get_attr_raw(bs, "position_y")
                .ok()
                .flatten()
                .unwrap_or(0),
        );

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(bs) => match bs.local_name().as_ref() {
                    "map" => {
                        self.map_visibility = TemplateVisibility::parse_map_attrs(&bs);
                    }
                    "templates" if !templates.is_empty() => {
                        Self::parse_template_visibilities(reader, templates)?;
                    }
                    _ => {}
                },
                Event::End(be) if be.local_name().as_ref() == "map_view" => break,
                Event::Eof => break,
                _ => {}
            }
        }

        Ok(())
    }

    fn parse_template_visibilities<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        templates: &mut Templates,
    ) -> Result<()> {
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(bs) if bs.local_name().as_ref() == "ref" => {
                    if let Ok(Some(index)) = try_get_attr_raw::<usize>(&bs, "template")
                        && index < templates.len()
                    {
                        templates.template_entries[index].visibility =
                            TemplateVisibility::parse_map_attrs(&bs);
                    }
                }
                Event::End(be) if be.local_name().as_ref() == "templates" => break,
                Event::Eof => break,
                _ => {}
            }
        }
        Ok(())
    }

    pub(crate) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        visibilities: Vec<TemplateVisibility>,
    ) -> Result<()> {
        let mut bs = BytesStart::new("view");
        if self.area_hatching_enabled {
            bs.push_attribute(("area_hatching_enabled", "true"));
        }
        if self.baseline_view_enabled {
            bs.push_attribute(("baseline_view_enabled", "true"));
        }
        writer.write_event(Event::Start(bs))?;

        self.grid.write(writer)?;

        let position_x = crate::xml::units::to_file_value(self.view_centre.x)?;
        let position_y = crate::xml::units::to_file_value(self.view_centre.y)?;
        let mut mv = BytesStart::new("map_view").with_attributes([
            ("zoom", format!("{:.4}", self.zoom.get()).as_str()),
            ("position_x", position_x.to_string().as_str()),
            ("position_y", position_y.to_string().as_str()),
        ]);

        if self.rotation != 0.0 {
            mv.push_attribute(("rotation", format!("{:.4}", self.rotation).as_str()));
        }

        writer.write_event(Event::Start(mv))?;

        writer.write_event(Event::Empty(BytesStart::new("map").with_attributes([
            (
                "opacity",
                format!("{:.2}", self.map_visibility.opacity.get()).as_str(),
            ),
            ("visible", self.map_visibility.visible.to_string().as_str()),
        ])))?;

        if visibilities.is_empty() {
            writer.write_event(Event::Empty(
                BytesStart::new("templates").with_attributes([("count", "0")]),
            ))?;
        } else {
            writer
                .write_event(Event::Start(BytesStart::new("templates").with_attributes(
                    [("count", visibilities.len().to_string().as_str())],
                )))?;

            for (index, vis) in visibilities.into_iter().enumerate() {
                writer.write_event(Event::Empty(BytesStart::new("ref").with_attributes([
                    ("template", index.to_string().as_str()),
                    ("opacity", format!("{:.2}", vis.opacity.get()).as_str()),
                    ("visible", vis.visible.to_string().as_str()),
                ])))?;
            }
            writer.write_event(Event::End(BytesEnd::new("templates")))?;
        }

        writer.write_event(Event::End(BytesEnd::new("map_view")))?;

        writer.write_event(Event::End(BytesEnd::new("view")))?;

        Ok(())
    }
}
