//! Pack rows use the existing main-thread ICU boundary, without library handles
//! escaping into workers. Culture ties retain deterministic discovery order.
use crate::editor_collation::Collator;
use deadsync_editor::{
    Error,
    pack::song_list::{OrderKey, Orderer},
};
pub(crate) struct NativePackSongOrder;
impl Orderer for NativePackSongOrder {
    fn order(&mut self, keys: &[OrderKey]) -> Result<Vec<usize>, Error> {
        let language = deadsync_theme_simply_love::i18n::current_locale();
        let collator =
            Collator::for_language(&language).map_err(|e| Error::InvalidEdit(e.to_string()))?;
        order_with(&collator, keys)
    }
}
fn order_with(collator: &Collator, keys: &[OrderKey]) -> Result<Vec<usize>, Error> {
    let mut keyed = keys
        .iter()
        .enumerate()
        .map(|(index, key)| {
            Ok((
                collator.sort_key(&key.title)?,
                collator.sort_key(&key.folder)?,
                index,
            ))
        })
        .collect::<Result<Vec<_>, crate::editor_collation::Error>>()
        .map_err(|e| Error::InvalidEdit(e.to_string()))?;
    keyed.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    Ok(keyed.into_iter().map(|(_, _, index)| index).collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pack_order_matches_real_original_for_both_recorded_cultures() {
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../deadsync-editor/tests/fixtures/grooveauthor/pack-songs.expected.json"
        ))
        .unwrap();
        let songs: Vec<_> = expected["Songs"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|song| song["Loaded"] == true)
            .collect();
        let keys: Vec<_> = songs
            .iter()
            .map(|song| {
                let title = song["TitleTranslit"]
                    .as_str()
                    .filter(|value| !value.is_empty())
                    .or_else(|| song["Title"].as_str())
                    .unwrap_or("");
                OrderKey {
                    title: title.into(),
                    folder: song["Name"].as_str().unwrap().into(),
                    path: song["Name"].as_str().unwrap().into(),
                }
            })
            .collect();
        for culture in ["en-US", "fr-FR"] {
            let collator = Collator::for_language(culture).unwrap();
            let indices = order_with(&collator, &keys).unwrap();
            let names: Vec<_> = indices
                .iter()
                .map(|&index| songs[index]["Name"].as_str().unwrap())
                .collect();
            assert_eq!(
                serde_json::json!(names),
                expected["Orders"][culture],
                "{culture}, ICU {:?}",
                collator.collation_version()
            );
        }
    }
}
