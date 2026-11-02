//! RSSP has no external timing-source API. Project only the requested chart into
//! an analysis-only SSC snapshot when an explicit native SM owner is active.
//! No output policy, simfile publication, format promotion or Session edit occurs.
use crate::{
    Error,
    document::{Document, Format},
    history::Session,
    timing_edit::{self, resident::Properties},
};
pub(super) struct Projection {
    pub bytes: Vec<u8>,
    description: String,
    difficulty: String,
    local_attacks: bool,
}
impl Projection {
    pub fn restore_sm_metadata(&self, summary: &mut rssp::ChartSummary) {
        // RSSP's SM description is also its step artist. SSC CREDIT would parse
        // tech notation twice if populated with the same description, so restore
        // only the presentation field after the normal dependency pass.
        summary.step_artist_str = self.description.clone();
        summary.difficulty_str = self.difficulty.clone();
        if self.local_attacks {
            summary.chart_has_own_attacks = true;
        }
    }
}
fn tag(out: &mut Vec<u8>, name: &str, raw: &[u8]) {
    out.push(b'#');
    out.extend_from_slice(name.as_bytes());
    out.push(b':');
    out.extend_from_slice(raw);
    out.extend_from_slice(b";\n");
}
pub(super) fn project(
    bytes: &[u8],
    chart: usize,
    entry: &rssp::parse::ParsedChartEntry<'_>,
) -> Result<Option<Projection>, Error> {
    if !bytes
        .windows(b"DSTIMING".len())
        .any(|part| part.eq_ignore_ascii_case(b"DSTIMING"))
        && !bytes
            .windows(b"DSCHARTATTACKS".len())
            .any(|part| part.eq_ignore_ascii_case(b"DSCHARTATTACKS"))
    {
        return Ok(None);
    }
    let document = Document::parse(bytes, Format::Sm)?;
    let local = timing_edit::resident::has_local(&document, chart);
    let song_tag = timing_edit::resident::tag(None)?;
    let local_attacks = crate::attacks::property::read(&document, Some(chart))?;
    if !local && document.raw_property(None, &song_tag).is_none() && local_attacks.is_none() {
        return Ok(None);
    }
    let properties = Properties::capture(&document, local.then_some(chart))?;
    let session = Session::new(document.clone());
    let chart_properties = crate::chart_properties::Draft::capture(&session, chart)?;
    let mut out = Vec::with_capacity(entry.note_data.len().saturating_add(4096));
    tag(&mut out, "VERSION", b"0.83");
    for source in document
        .tags()
        .iter()
        .filter(|source| source.chart.is_none())
    {
        let name = source.name.as_str();
        if name == "VERSION"
            || name == "FREEZES"
            || name == "DISPLAYBPM"
            || timing_edit::TIMING_TAGS.contains(&name)
            || name.starts_with("DSTIMING")
            || name.starts_with("DSATTACKROWS")
            || name.starts_with("DSCHARTATTACKS")
            || (name == "ATTACKS" && local_attacks.is_some())
            || name.starts_with("GACHARTDATA")
            || name.starts_with("GASONGDATA")
        {
            continue;
        }
        tag(&mut out, name, document.value(source.value.clone()));
    }
    let write_timing = |out: &mut Vec<u8>| {
        for name in timing_edit::TIMING_TAGS {
            let value = if name == "OFFSET" {
                chart_properties.effective_music_offset().to_string()
            } else {
                properties.text(name)
            };
            tag(out, name, &crate::document::escape_text(&value));
        }
        tag(
            out,
            "DISPLAYBPM",
            // The typed serializer emits only numeric tokens, ':' or '*'.
            // SSC display ranges use a literal separator; RSSP retains a
            // backslash here instead of decoding it as simfile text.
            chart_properties.display_tempo().serialize().as_bytes(),
        );
    };
    if !local {
        write_timing(&mut out);
    }
    tag(&mut out, "NOTEDATA", b"");
    for (name, raw) in [
        ("STEPSTYPE", entry.fields[0]),
        ("DESCRIPTION", entry.fields[1]),
        ("DIFFICULTY", entry.fields[2]),
        ("METER", entry.fields[3]),
    ] {
        tag(&mut out, name, raw);
    }
    tag(&mut out, "CREDIT", b"");
    if let Some(raw) = &local_attacks {
        tag(&mut out, "ATTACKS", raw);
    }
    // RSSP requires two complete radar sets for SSC but one for SM. Repeating
    // the raw SM set preserves its exact first-set validity and values without
    // introducing a second radar parser or recomputing cached radar values.
    let mut radar = entry.fields[4].to_vec();
    radar.extend_from_slice(b"\n,");
    radar.extend_from_slice(entry.fields[4]);
    tag(&mut out, "RADARVALUES", &radar);
    if local {
        write_timing(&mut out);
    }
    tag(&mut out, "NOTES", entry.note_data);
    let description = rssp::parse::decode_unescape_trim(entry.fields[1]).into_owned();
    let raw_difficulty = rssp::parse::decode_unescape_trim(entry.fields[2]);
    let rating = rssp::parse::decode_unescape_trim(entry.fields[3]);
    let difficulty = rssp::resolve_difficulty_label(&raw_difficulty, &description, &rating, "sm");
    Ok(Some(Projection {
        bytes: out,
        description,
        difficulty,
        local_attacks: local_attacks.is_some(),
    }))
}
