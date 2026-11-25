//! The `<georeferencing>` section.

use crate::geo_referencing::{CrsType, GeoRef};
use crate::xml::attrs::try_get_attr_raw;
use crate::xml::{Error, Result, Section};
use geo_types::Coord;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::str::FromStr as _;

impl GeoRef {
    pub(in crate::xml) fn write<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut bytes_start = BytesStart::new("georeferencing")
            .with_attributes([("scale", self.scale_denominator.to_string().as_str())]);
        if self.combined_scale_factor() != 1. {
            bytes_start.push_attribute((
                "grid_scale_factor",
                self.combined_scale_factor().to_string().as_str(),
            ));
        }
        if self.auxiliary_scale_factor.get() != 1. {
            bytes_start.push_attribute((
                "auxiliary_scale_factor",
                self.auxiliary_scale_factor.get().to_string().as_str(),
            ));
        }
        if self.declination_deg != 0. {
            bytes_start.push_attribute(("declination", self.declination_deg.to_string().as_str()));
        }
        if self.grivation_deg() != 0. {
            bytes_start.push_attribute(("grivation", self.grivation_deg().to_string().as_str()));
        }

        writer.write_event(Event::Start(bytes_start))?;
        if self.map_ref_point != Coord::zero() {
            // for some reason in mm and not µm, but y is flipped
            writer.write_event(Event::Empty(BytesStart::new("ref_point").with_attributes(
                [
                    ("x", self.map_ref_point.x.to_string().as_str()),
                    ("y", (-self.map_ref_point.y).to_string().as_str()),
                ],
            )))?;
        }

        let is_local_crs = matches!(self.crs_type, CrsType::Local);
        self.crs_type.write(writer)?;
        if self.projected_ref_point != Coord::zero() {
            writer.write_event(Event::Empty(BytesStart::new("ref_point").with_attributes(
                [
                    ("x", self.projected_ref_point.x.to_string().as_str()),
                    ("y", self.projected_ref_point.y.to_string().as_str()),
                ],
            )))?;
        }
        writer.write_event(Event::End(BytesEnd::new("projected_crs")))?;

        if !is_local_crs {
            writer.write_event(Event::Start(
                BytesStart::new("geographic_crs")
                    .with_attributes([("id", "Geographic coordinates")]),
            ))?;
            writer.write_event(Event::Start(
                BytesStart::new("spec").with_attributes([("language", "PROJ.4")]),
            ))?;
            writer.write_event(Event::Text(BytesText::new("+proj=latlong +datum=WGS84")))?;
            writer.write_event(Event::End(BytesEnd::new("spec")))?;
            writer.write_event(Event::Empty(
                BytesStart::new("ref_point_deg").with_attributes([
                    ("lat", self.geographic_ref_point_deg.y.to_string().as_str()),
                    ("lon", self.geographic_ref_point_deg.x.to_string().as_str()),
                ]),
            ))?;
            writer.write_event(Event::End(BytesEnd::new("geographic_crs")))?;
        }

        writer.write_event(Event::End(BytesEnd::new("georeferencing")))?;
        Ok(())
    }

    pub(in crate::xml) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        event: &BytesStart<'_>,
    ) -> Result<Self> {
        let scale = try_get_attr_raw(event, "scale")?.ok_or(Error::MissingMapScale)?;
        let auxiliary_scale_factor = try_get_attr_raw(event, "auxiliary_scale_factor")
            .ok()
            .flatten()
            .unwrap_or(1.);
        let grid_scale_factor = try_get_attr_raw(event, "grid_scale_factor")
            .ok()
            .flatten()
            .unwrap_or(1.)
            / auxiliary_scale_factor;
        let declination_deg = try_get_attr_raw(event, "declination")?.unwrap_or(0.);
        let convergence_deg = declination_deg - try_get_attr_raw(event, "grivation")?.unwrap_or(0.);

        let mut crs_type = CrsType::Local;
        let mut map_ref_point = Coord::zero();
        let mut projected_ref_point = Coord::zero();
        let mut geographic_ref_point_deg = Coord::zero();

        let mut buf = Vec::new();
        loop {
            let event = reader.read_event_into(&mut buf)?;

            match event {
                Event::Start(bs) => match bs.local_name().as_ref() {
                    "projected_crs" => {
                        (crs_type, projected_ref_point) = parse_projected_crs(reader, &bs)?;
                    }
                    "geographic_crs" => geographic_ref_point_deg = parse_geographic_crs(reader)?,
                    "ref_point" => {
                        // for some reason in mm and not µm, but y is flipped
                        map_ref_point = Coord {
                            x: try_get_attr_raw(&bs, "x")?.unwrap_or(map_ref_point.x),
                            y: try_get_attr_raw(&bs, "y")?
                                .map(|y: f64| -y)
                                .unwrap_or(map_ref_point.y),
                        }
                    }
                    _ => (),
                },
                Event::End(bytes_end) => {
                    if matches!(bytes_end.local_name().as_ref(), "georeferencing") {
                        break;
                    }
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(Section::Georeferencing));
                }
                _ => (),
            }
        }

        Ok(Self {
            scale_denominator: scale,
            grid_scale_factor: grid_scale_factor.try_into()?,
            auxiliary_scale_factor: auxiliary_scale_factor.try_into()?,
            declination_deg,
            convergence_deg,
            crs_type,
            map_ref_point,
            projected_ref_point,
            geographic_ref_point_deg,
        })
    }
}

fn parse_projected_crs<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    bytes_start: &BytesStart<'_>,
) -> Result<(CrsType, Coord)> {
    let mut buf = Vec::new();

    let crs_type = if let Some(attr) = bytes_start.try_get_attribute("id")? {
        match attr.value.as_ref() {
            "Gauss-Krueger, datum: Potsdam" => {
                let param_string = get_projected_crs_spec(reader, "parameter")?;
                CrsType::GaussKrueger(param_string.parse()?)
            }
            "EPSG" => {
                let param_string = get_projected_crs_spec(reader, "parameter")?;
                CrsType::Epsg(u16::from_str(param_string.as_str())?)
            }
            "UTM" => {
                let param_string = get_projected_crs_spec(reader, "parameter")?;
                CrsType::Utm(param_string.parse()?)
            }
            "Local" => CrsType::Local,
            _ => {
                let spec_string = get_projected_crs_spec(reader, "spec")?;
                CrsType::Proj4(spec_string)
            }
        }
    } else {
        let spec_string = get_projected_crs_spec(reader, "spec")?;
        CrsType::Proj4(spec_string)
    };

    let mut proj_ref_point = Coord::zero();
    loop {
        let event = reader.read_event_into(&mut buf)?;

        match event {
            Event::Start(bs) => {
                if matches!(bs.local_name().as_ref(), "ref_point") {
                    proj_ref_point = Coord {
                        x: try_get_attr_raw(&bs, "x")?.unwrap_or(proj_ref_point.x),
                        y: try_get_attr_raw(&bs, "y")?.unwrap_or(proj_ref_point.y),
                    }
                }
            }
            Event::End(bytes_end) => {
                if matches!(bytes_end.local_name().as_ref(), "projected_crs") {
                    break;
                }
            }
            Event::Eof => {
                return Err(Error::UnexpectedEof(Section::Georeferencing));
            }
            _ => (),
        }
    }
    Ok((crs_type, proj_ref_point))
}

fn parse_geographic_crs<R: std::io::BufRead>(reader: &mut Reader<R>) -> Result<Coord> {
    let mut buf = Vec::new();

    let mut geo_ref_point = Coord::zero();
    loop {
        let event = reader.read_event_into(&mut buf)?;

        match event {
            Event::Start(bs) => {
                if matches!(bs.local_name().as_ref(), "ref_point_deg") {
                    geo_ref_point = Coord {
                        x: try_get_attr_raw(&bs, "lon")?.unwrap_or(geo_ref_point.x),
                        y: try_get_attr_raw(&bs, "lat")?.unwrap_or(geo_ref_point.y),
                    }
                }
            }
            Event::End(bytes_end) => {
                if matches!(bytes_end.local_name().as_ref(), "geographic_crs") {
                    break;
                }
            }
            Event::Eof => {
                return Err(Error::UnexpectedEof(Section::Georeferencing));
            }
            _ => (),
        }
    }
    Ok(geo_ref_point)
}

fn get_projected_crs_spec<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    event_name: &str,
) -> Result<String> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(bytes_start) if bytes_start.local_name().as_ref() == event_name => {
                return crate::xml::read_text(reader);
            }
            Event::Eof => {
                return Err(Error::UnexpectedEof(Section::Georeferencing));
            }
            _ => (),
        }
    }
}

impl CrsType {
    pub(in crate::xml) fn write<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let (id, proj_str, parameter) = match &self {
            Self::Local => {
                writer.write_event(Event::Start(
                    BytesStart::new("projected_crs").with_attributes([("id", "Local")]),
                ))?;
                return Ok(());
            }
            Self::Epsg(code) => ("EPSG", format!("+init=epsg:{code}"), format!("{code}")),
            Self::Proj4(proj_string) => ("PROJ.4", proj_string.clone(), proj_string.clone()),
            Self::GaussKrueger(code) => {
                let lon = 3 * (code.get() as u16);
                let x = 500_000 + (code.get() as u32 * 1_000_000);
                (
                    "Gauss-Krueger, datum: Potsdam",
                    format!(
                        "+proj=tmerc +lat_0=0 +lon_0={lon} +k=1.000000 +x_0={x} +y_0=0 +ellps=bessel +datum=potsdam +units=m +no_defs"
                    ),
                    format!("{}", code.get()),
                )
            }
            Self::Utm(code) => {
                let (proj_str, param_str) = if code.get() < 0 {
                    (
                        format!("+proj=utm +datum=WGS84 +zone={} +south", code.get().abs()),
                        format!("{} S", code.get().abs()),
                    )
                } else {
                    (
                        format!("+proj=utm +datum=WGS84 +zone={}", code.get().abs()),
                        format!("{} N", code.get().abs()),
                    )
                };
                ("UTM", proj_str, param_str)
            }
        };
        writer.write_event(Event::Start(
            BytesStart::new("projected_crs").with_attributes([("id", id)]),
        ))?;
        writer.write_event(Event::Start(
            BytesStart::new("spec").with_attributes([("language", "PROJ.4")]),
        ))?;
        writer.write_event(Event::Text(BytesText::new(&proj_str)))?;
        writer.write_event(Event::End(BytesEnd::new("spec")))?;
        writer.write_event(Event::Start(BytesStart::new("parameter")))?;
        writer.write_event(Event::Text(BytesText::new(&parameter)))?;
        writer.write_event(Event::End(BytesEnd::new("parameter")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use geo_types::Coord;
    use quick_xml::Reader;

    use crate::geo_referencing::{CrsType, GeoRef};
    use crate::{PositiveF64, geo_referencing::UtmCode};

    #[test]
    fn parse_utm() {
        let geo_tag = "<georeferencing scale=\"10000\" auxiliary_scale_factor=\"1.000352\" declination=\"3.29\" grivation=\"2.16\"><projected_crs id=\"UTM\"><spec language=\"PROJ.4\">+proj=utm +datum=WGS84 +zone=32</spec><parameter>32</parameter><ref_point x=\"563000\" y=\"7032000\"/></projected_crs><geographic_crs id=\"Geographic coordinates\"><spec language=\"PROJ.4\">+proj=latlong +datum=WGS84</spec><ref_point_deg lat=\"63.41097085\" lon=\"10.26159582\"/></geographic_crs></georeferencing>";

        let mut reader = Reader::from_str(geo_tag);
        reader.config_mut().expand_empty_elements = true;

        let actual = match reader
            .read_event()
            .expect("read the georeferencing start tag")
        {
            quick_xml::events::Event::Start(bytes_start) => {
                GeoRef::parse(&mut reader, &bytes_start).expect("Test failed")
            }
            _ => unreachable!("Should not be reached"),
        };
        let expected = GeoRef {
            scale_denominator: NonZeroU32::new(10_000).expect("the scale is nonzero"),
            grid_scale_factor: PositiveF64::try_from(1. / 1.000352)
                .expect("the grid scale factor is positive"),
            auxiliary_scale_factor: PositiveF64::try_from(1.000352)
                .expect("the auxiliary scale factor is positive"),
            declination_deg: 3.29,
            convergence_deg: 3.29 - 2.16,
            crs_type: CrsType::Utm(UtmCode::new(32).expect("32 is a valid UTM zone")),
            map_ref_point: Coord { x: 0., y: 0. },
            projected_ref_point: Coord {
                x: 563_000.,
                y: 7_032_000.,
            },
            geographic_ref_point_deg: Coord {
                x: 10.26159582,
                y: 63.41097085,
            },
        };

        assert_eq!(actual, expected);
    }
}
