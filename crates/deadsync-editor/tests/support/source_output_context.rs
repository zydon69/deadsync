//! Independent expected-byte builder for the two source song-output properties.
//! Context is explicit for fixtures with different timing layout/index/sync.
//! No production output-policy/preflight/materialization function is called.
use deadsync_editor::document::{Document, Format, escape_text};
use serde_json::{Map, Value};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub sync_offset: f64,
    pub chart_type: &'static str,
    pub chart_index: i32,
}
const DEFAULT: Context = Context {
    sync_offset: 0.009,
    chart_type: "dance_single",
    chart_index: 0,
};

// Fumen resolves Version/Data physical aliases independently before loading.
// Native lossless output preserves the selected existing spelling.
fn property_key(document: &Document, base: &str) -> String {
    let marked = format!("{base}SONG");
    document
        .tags()
        .iter()
        .filter(|tag| {
            tag.chart.is_none()
                && (tag.name.eq_ignore_ascii_case(base)
                    || document.format == Format::Sm && tag.name.eq_ignore_ascii_case(&marked))
        })
        .max_by_key(|tag| tag.value.start)
        .map(|tag| tag.name.clone())
        .unwrap_or_else(|| {
            if document.format == Format::Sm {
                marked
            } else {
                base.to_owned()
            }
        })
}

pub fn expected(resident: &[u8], format: Format) -> Vec<u8> {
    expected_with_context(resident, format, DEFAULT)
}
pub fn expected_with_context(resident: &[u8], format: Format, context: Context) -> Vec<u8> {
    let document = Document::parse(Arc::<[u8]>::from(resident), format).unwrap();
    let data_name = property_key(&document, "GASongData");
    let version_name = property_key(&document, "GASongDataVersion");
    let mut metadata: Map<String, Value> = document
        .text(None, &data_name)
        .map(|text| serde_json::from_str(&text).unwrap())
        .unwrap_or_default();
    metadata
        .entry("SyncOffset")
        .or_insert(Value::from(context.sync_offset));
    assert_eq!(
        metadata["SyncOffset"].as_f64(),
        Some(context.sync_offset),
        "fixture must declare its actual sync context"
    );
    metadata.insert("DefaultChartType".into(), Value::from(context.chart_type));
    metadata.insert("DefaultChartIndex".into(), Value::from(context.chart_index));
    let values = [
        (
            data_name,
            escape_text(&serde_json::to_string(&metadata).unwrap()),
        ),
        (version_name, b"1".to_vec()),
    ];
    let mut replacements = BTreeMap::new();
    let mut inserted = Vec::new();
    for (name, value) in values {
        if let Some(tag) = document.tag(None, &name) {
            replacements.insert((tag.value.start, tag.value.end), value);
        } else {
            inserted.extend_from_slice(format!("\n#{name}:").as_bytes());
            inserted.extend_from_slice(&value);
            inserted.extend_from_slice(b";\n");
        }
    }
    if !inserted.is_empty() {
        let at = document
            .tags()
            .iter()
            .find(|tag| tag.chart.is_some())
            .map_or(resident.len(), |tag| {
                resident[..tag.value.start]
                    .iter()
                    .rposition(|&byte| byte == b'#')
                    .unwrap()
            });
        replacements.insert((at, at), inserted);
    }
    let mut output = resident.to_vec();
    for ((start, end), value) in replacements.into_iter().rev() {
        output.splice(start..end, value);
    }
    output
}

pub fn assert_context_only(resident: &[u8], published: &[u8], format: Format) {
    assert_context(resident, published, format, DEFAULT);
}
pub fn assert_context(resident: &[u8], published: &[u8], format: Format, context: Context) {
    // Exact bytes retain every other tag, note, peer, comment, encoding and space.
    assert_eq!(published, expected_with_context(resident, format, context));
    let document = Document::parse(Arc::<[u8]>::from(published), format).unwrap();
    assert_eq!(
        document
            .text(None, &property_key(&document, "GASongDataVersion"))
            .as_deref(),
        Some("1")
    );
    let metadata: Value = serde_json::from_str(
        &document
            .text(None, &property_key(&document, "GASongData"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(metadata["DefaultChartType"], context.chart_type);
    assert_eq!(metadata["DefaultChartIndex"], context.chart_index);
    assert_eq!(metadata["SyncOffset"].as_f64(), Some(context.sync_offset));
}
