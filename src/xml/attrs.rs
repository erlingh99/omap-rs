//! Reading typed attribute values.

use crate::xml::{Error, Result};
use quick_xml::XmlVersion;
use quick_xml::events::BytesStart;
use quick_xml::events::attributes::Attribute;
use std::str::FromStr;

/// Do not use this for fields with user text as it will not unescape xml-codes
pub(crate) fn parse_attr_raw<T: FromStr>(value: std::borrow::Cow<'_, str>) -> Result<T> {
    let e = match T::from_str(&value) {
        Ok(it) => it,
        Err(_err) => return Err(Error::FromStrError),
    };
    Ok(e)
}

/// Do not use this for fields with user text as it will not unescape xml-codes
pub(crate) fn try_get_attr_raw<T: FromStr>(
    bytes: &BytesStart<'_>,
    attr: &str,
) -> Result<Option<T>> {
    let a = bytes
        .try_get_attribute(attr)?
        .map(|a| parse_attr_raw(a.value));
    a.transpose()
}

/// This escapes any xml-codes, use for user-strings
pub(crate) fn parse_attr<T: FromStr>(attr: Attribute<'_>) -> Result<T> {
    let e = match T::from_str(&attr.normalized_value(XmlVersion::Explicit1_0)?) {
        Ok(it) => it,
        Err(_err) => return Err(Error::FromStrError),
    };
    Ok(e)
}

/// This escapes any xml-codes, use for user-strings
pub(crate) fn try_get_attr<T: FromStr>(bytes: &BytesStart<'_>, attr: &str) -> Result<Option<T>> {
    let a = bytes.try_get_attribute(attr)?.map(|a| parse_attr(a));
    a.transpose()
}
