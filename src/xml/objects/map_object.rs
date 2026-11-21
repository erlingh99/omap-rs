//! The `<object>` element, whichever kind it holds.

use crate::objects::{
    AreaObject, HorizontalAlign, LineObject, MapObject, PointObject, TextObject, VerticalAlign,
};
use crate::symbols::{
    AreaPathSymbolId, LinePathSymbolId, PointSymbolId, SymbolId, SymbolSet, TextSymbolId,
};
use crate::xml::attrs::parse_attr_raw;
use crate::{Error, Result};
use quick_xml::events::BytesStart;
use quick_xml::{Reader, Writer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(super) enum ObjectType {
    Point,
    Line,
    Area,
    Text,
}

/// Narrow the handle an object names to the kind that object can render.
fn narrow<'a, T>(
    symbol: Option<crate::Entry<'a, SymbolId, crate::symbols::Symbol>>,
) -> Result<Option<T>>
where
    T: TryFrom<crate::Entry<'a, SymbolId, crate::symbols::Symbol>, Error = Error>,
{
    symbol.map(T::try_from).transpose()
}

impl MapObject {
    pub(crate) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        symbol_set: &SymbolSet,
    ) -> Result<()> {
        if self.geometry_is_empty() {
            return Ok(());
        }

        match self {
            Self::Point(point_object) => point_object.write(writer, symbol_set)?,
            Self::Line(line_object) => line_object.write(writer, symbol_set)?,
            Self::Area(area_object) => area_object.write(writer, symbol_set)?,
            Self::Text(text_object) => text_object.write(writer, symbol_set)?,
        }
        Ok(())
    }
}

impl MapObject {
    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        bytes_start: &BytesStart<'_>,
        symbols: &SymbolSet,
        is_line_element: bool,
    ) -> Result<Self> {
        let mut object_type = None;
        let mut symbol_id = None;
        let mut rotation = 0.;
        let mut h_align = HorizontalAlign::default();
        let mut v_align = VerticalAlign::default();

        for attr in bytes_start.attributes().filter_map(std::result::Result::ok) {
            match attr.key.local_name().as_ref() {
                "type" => match attr.value.as_ref() {
                    "0" => object_type = Some(ObjectType::Point),
                    "1" => object_type = Some(ObjectType::Area),
                    "4" => object_type = Some(ObjectType::Text),
                    _ => (),
                },
                "symbol" => symbol_id = parse_attr_raw::<i32>(attr.value).ok(),
                "rotation" => rotation = parse_attr_raw(attr.value).unwrap_or(rotation),
                "h_align" => h_align = parse_attr_raw(attr.value).unwrap_or(h_align),
                "v_align" => v_align = parse_attr_raw(attr.value).unwrap_or(v_align),
                _ => (),
            }
        }

        let Some(mut object_type) = object_type else {
            return Err(Error::MissingObjectType);
        };

        if is_line_element {
            object_type = ObjectType::Line;
        }

        // An inline element symbol, or the format's -1, both mean `None`.
        let symbol = if let Some(sid) = symbol_id
            && sid >= 0
        {
            Some(
                symbols
                    .at(usize::try_from(sid)?)
                    .ok_or(Error::UnknownObjectSymbolId(sid))?,
            )
        } else {
            None
        };

        // Mapper does not distinguish area from line objects; the symbol does.
        if object_type == ObjectType::Area
            && symbol.is_some_and(|symbol| LinePathSymbolId::try_from(symbol).is_ok())
        {
            object_type = ObjectType::Line;
        }

        match object_type {
            ObjectType::Point => Ok(Self::Point(PointObject::parse(
                reader,
                narrow::<PointSymbolId>(symbol)?,
                rotation,
            )?)),
            ObjectType::Line => Ok(Self::Line(LineObject::parse(
                reader,
                narrow::<LinePathSymbolId>(symbol)?,
            )?)),
            // do not bother sending rotation to the AreaObject as it is also given in the pattern rotation
            ObjectType::Area => Ok(Self::Area(AreaObject::parse(
                reader,
                narrow::<AreaPathSymbolId>(symbol)?,
            )?)),
            ObjectType::Text => Ok(Self::Text(TextObject::parse(
                reader,
                narrow::<TextSymbolId>(symbol)?,
                h_align,
                v_align,
                rotation,
            )?)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use geo_types::{LineString, Point, Polygon, coord};

    use super::MapObject;
    use crate::{
        Result,
        objects::{AreaObject, PointObject, TextGeometry, TextObject},
    };

    #[test]
    fn rotatable_objects_follow_transform_rotation() -> Result<()> {
        let transform = |coord: geo_types::Coord| coord! { x: -coord.y + 10., y: coord.x - 5. };

        let mut point = PointObject::new(None, Point::new(1., 2.));
        point.rotation = 0.1;
        let mut point = MapObject::Point(point);
        point.transform(transform);
        let MapObject::Point(point) = point else {
            return Err(crate::Error::ObjectError);
        };
        assert!((point.rotation - (0.1 + FRAC_PI_2)).abs() < 1e-12);

        let mut text = TextObject::new(
            None,
            TextGeometry::SingleAnchor(coord! { x: 1., y: 2. }),
            String::new(),
        );
        text.rotation = -0.2;
        let mut text = MapObject::Text(text);
        text.transform(transform);
        let MapObject::Text(text) = text else {
            return Err(crate::Error::ObjectError);
        };
        assert!((text.rotation - (-0.2 + FRAC_PI_2)).abs() < 1e-12);

        let ring = LineString::from(vec![
            coord! { x: 0., y: 0. },
            coord! { x: 1., y: 0. },
            coord! { x: 0., y: 1. },
            coord! { x: 0., y: 0. },
        ]);
        let mut area = AreaObject::new(None, Polygon::new(ring, Vec::new()));
        area.pattern_rotation.coord = coord! { x: 1., y: 2. };
        area.pattern_rotation.rotation = 0.3;
        let mut area = MapObject::Area(area);
        area.transform(transform);
        let MapObject::Area(area) = area else {
            return Err(crate::Error::ObjectError);
        };
        assert!((area.pattern_rotation.rotation - (0.3 + FRAC_PI_2)).abs() < 1e-12);

        Ok(())
    }
}
