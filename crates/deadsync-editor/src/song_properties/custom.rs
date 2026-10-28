//! Fumen SMReader: bare and SONG-marked properties share one logical Extra.
//! The last physical occurrence wins independently for Version and Data.
use crate::document::{Document, Format};

pub(super) fn key(document: &Document, name: &str) -> String {
    if document.format != Format::Sm {
        return name.into();
    }
    let marked = format!("{name}SONG");
    let insertion = document
        .tags()
        .iter()
        .find(|tag| tag.chart.is_some())
        .map_or(document.source().len(), |tag| {
            document.source()[..tag.value.start]
                .iter()
                .rposition(|b| *b == b'#')
                .unwrap_or(0)
        });
    let mut candidates = Vec::new();
    for candidate in [name, &marked] {
        if let Some(tag) = document.tag(None, candidate) {
            candidates.push((tag.value.start, 0, candidate.to_owned()));
        }
        if document.added_property(None, candidate).is_some() {
            candidates.push((insertion, 1, candidate.to_owned()));
        }
    }
    candidates
        .into_iter()
        .max()
        .map_or(marked, |(_, _, key)| key)
}
pub(super) fn text(document: &Document, name: &str) -> Option<String> {
    document.text(None, &key(document, name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    fn decode(s: &str) -> Vec<u8> {
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let (mut bits, mut count, mut out) = (0u32, 0, Vec::new());
        for b in s.bytes().take_while(|v| *v != b'=') {
            bits = (bits << 6) | alphabet.iter().position(|v| *v == b).unwrap() as u32;
            count += 6;
            if count >= 8 {
                count -= 8;
                out.push((bits >> count) as u8);
                bits &= (1 << count) - 1;
            }
        }
        out
    }
    #[test]
    fn original_reader_twenty_four_cases_resolve_each_field_independently() {
        let request: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/grooveauthor/song-custom.request.json"
        ))
        .unwrap();
        let expected: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/grooveauthor/song-custom.expected.json"
        ))
        .unwrap();
        let mut adaptations = 0;
        for (input, result) in request["Cases"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["Cases"].as_array().unwrap())
        {
            assert_eq!(input["Name"], result["Name"]);
            let name = input["Name"].as_str().unwrap();
            let format = if input["Format"] == "sm" {
                Format::Sm
            } else {
                Format::Ssc
            };
            let document =
                Document::parse(decode(input["Bytes"].as_str().unwrap()), format).unwrap();
            if name.starts_with("lower-base") || name == "lower-marked-sm" {
                assert_eq!(text(&document, "GASongDataVersion").as_deref(), Some("1"));
                adaptations += 1;
                continue;
            }
            for (field, key) in [("GASongDataVersion", "Version"), ("GASongData", "Data")] {
                assert_eq!(
                    text(&document, field).as_deref(),
                    result[key].as_str(),
                    "{name}:{field}"
                );
            }
        }
        assert_eq!(adaptations, 3);
    }
}
