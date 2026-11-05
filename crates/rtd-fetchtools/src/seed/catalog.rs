//! `catalogfetch` seed values: countries, timezones, emoji groups and keywords.
//!
//! The JSON shapes mirror the Go `cmd/catalogfetch` structs, including their
//! `omitempty` behaviour, so the files are byte-identical.

use rtd_tg::{
    EmojiGroup, EmojiKeyword, EmojiKeywordsDifference, HelpCountriesList, HelpTimezonesList,
    Timezone,
};

use crate::gojson::Value;

/// Builds `countries.json` from `help.getCountriesList`.
#[must_use]
pub fn countries(list: &HelpCountriesList) -> Value {
    let HelpCountriesList::CountriesList { countries, .. } = list else {
        return Value::object(vec![("countries", Value::Array(Vec::new()))]);
    };
    let entries = countries
        .iter()
        .map(|country| {
            let codes = country
                .country_codes
                .iter()
                .map(|code| {
                    let mut fields: Vec<(&str, Value)> =
                        vec![("country_code", Value::Str(code.country_code.clone()))];
                    push_optional_strings(&mut fields, "prefixes", code.prefixes.as_ref());
                    push_optional_strings(&mut fields, "patterns", code.patterns.as_ref());
                    Value::object(fields)
                })
                .collect();
            let mut fields: Vec<(&str, Value)> = vec![
                ("iso2", Value::Str(country.iso2.clone())),
                ("default_name", Value::Str(country.default_name.clone())),
            ];
            if let Some(name) = &country.name
                && !name.is_empty()
            {
                fields.push(("name", Value::Str(name.clone())));
            }
            if country.hidden {
                fields.push(("hidden", Value::Bool(true)));
            }
            fields.push(("country_codes", Value::Array(codes)));
            Value::object(fields)
        })
        .collect();
    Value::object(vec![("countries", Value::Array(entries))])
}

/// Builds `timezones.json` from `help.getTimezonesList`.
#[must_use]
pub fn timezones(list: &HelpTimezonesList) -> Value {
    let HelpTimezonesList::TimezonesList { timezones, .. } = list else {
        return Value::object(vec![("timezones", Value::Array(Vec::new()))]);
    };
    let entries = timezones.iter().map(timezone_value).collect();
    Value::object(vec![("timezones", Value::Array(entries))])
}

/// Converts one [`Timezone`].
fn timezone_value(timezone: &Timezone) -> Value {
    Value::object(vec![
        ("id", Value::Str(timezone.id.clone())),
        ("name", Value::Str(timezone.name.clone())),
        ("utc_offset", Value::Int(i64::from(timezone.utc_offset))),
    ])
}

/// Builds `emoji_groups.json` from `messages.getEmojiGroups`.
#[must_use]
pub fn emoji_groups(groups: &[EmojiGroup]) -> Value {
    let entries = groups
        .iter()
        .filter_map(|group| match group {
            EmojiGroup::EmojiGroup {
                title,
                icon_emoji_id,
                emoticons,
            } => Some(Value::object(vec![
                ("title", Value::Str(title.clone())),
                ("icon_emoji_id", Value::Int(*icon_emoji_id)),
                ("emoticons", Value::from(emoticons.clone())),
            ])),
            _ => None,
        })
        .collect();
    Value::object(vec![("groups", Value::Array(entries))])
}

/// Builds `emoji_keywords_<lang>.json` from `messages.getEmojiKeywords`.
#[must_use]
pub fn emoji_keywords(difference: &EmojiKeywordsDifference) -> Value {
    let entries = difference
        .keywords
        .iter()
        .filter_map(|keyword| match keyword {
            EmojiKeyword::EmojiKeyword { keyword, emoticons } => Some(Value::object(vec![
                ("keyword", Value::Str(keyword.clone())),
                ("emoticons", Value::from(emoticons.clone())),
            ])),
            EmojiKeyword::EmojiKeywordDeleted { .. } => None,
        })
        .collect();
    Value::object(vec![
        ("lang_code", Value::Str(difference.lang_code.clone())),
        ("version", Value::Int(i64::from(difference.version))),
        ("keywords", Value::Array(entries)),
    ])
}

/// Appends a string-array field when it is present and non-empty (`omitempty`).
fn push_optional_strings(
    fields: &mut Vec<(&'static str, Value)>,
    key: &'static str,
    value: Option<&Vec<String>>,
) {
    if let Some(items) = value
        && !items.is_empty()
    {
        fields.push((key, Value::from(items.clone())));
    }
}
