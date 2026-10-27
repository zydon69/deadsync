//! Empty chart creation: original song defaults, no inherited chart payload.
use crate::{
    Error,
    document::{Document, Format},
    generation::{metadata::ChartData, timing::GenerationTiming},
    history::{RevisionToken, Session},
    notes::{Grid, layout_columns},
    timing_edit::signature::{Signature, Signatures},
};
pub const LAYOUTS: [&str; 16] = [
    "dance-single",
    "dance-double",
    "dance-couple",
    "dance-routine",
    "dance-solo",
    "dance-threepanel",
    "pump-single",
    "pump-halfdouble",
    "pump-double",
    "pump-couple",
    "pump-routine",
    "smx-beginner",
    "smx-single",
    "smx-dual",
    "smx-full",
    "smx-team",
];
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Defaults {
    pub tempo: f64,
    pub signature: Signature,
    pub song_offset: f64,
}
pub struct Draft {
    token: RevisionToken,
    defaults: Defaults,
}
impl Draft {
    pub fn capture(session: &Session) -> Result<Self, Error> {
        if !session.document().charts().is_empty()
            && crate::timing_edit::migration::needed(session.document())
        {
            return Err(invalid("Review legacy SSC timing before creating a chart"));
        }
        // Flush working timing/notes to an isolated source for stable descriptors.
        let document = Document::parse(session.serialize()?, session.document().format)?;
        let tempo = GenerationTiming::new_chart_tempo(&document)?;
        let signature = starting_signature(&document)?;
        let song_offset = document
            .text(None, "OFFSET")
            .map(|s| {
                s.trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .ok_or_else(|| invalid("Invalid song offset"))
            })
            .transpose()?
            .unwrap_or(0.0);
        Ok(Self {
            token: session.revision_token(),
            defaults: Defaults {
                tempo,
                signature,
                song_offset,
            },
        })
    }
    pub fn defaults(&self) -> Defaults {
        self.defaults
    }
    pub fn apply(&self, session: &mut Session, layout: &str) -> Result<usize, Error> {
        if !session.accepts_token(&self.token) {
            return Err(invalid(
                "New chart settings belong to another document or revision",
            ));
        }
        let layout = crate::notes::canonical_layout(layout)
            .ok_or_else(|| invalid("Unsupported new chart layout"))?;
        let columns = layout_columns(layout).ok_or_else(|| invalid("Unsupported new chart layout"))?;
        let players = crate::notes::default_players(layout).ok_or_else(|| invalid("Unknown new chart participants"))?;
        let grid = Grid::new(columns, players)?;
        let notes = grid.serialize()?;
        let mut block = if session.document().format == Format::Ssc {
            format!("#NOTEDATA:;\n#STEPSTYPE:{layout};\n#DESCRIPTION:;\n#DIFFICULTY:Beginner;\n#METER:1;\n#BPMS:0={};\n#TIMESIGNATURES:0={}={};\n#OFFSET:{};\n#SCROLLS:0=1;\n#SPEEDS:0=1=0=0;\n#TICKCOUNTS:0=4;\n#COMBOS:0=1=1;\n#NOTES:",self.defaults.tempo,self.defaults.signature.numerator,self.defaults.signature.denominator,self.defaults.song_offset).into_bytes()
        } else {
            format!("#NOTES:{layout}::Beginner:1:0,0,0,0,0:").into_bytes()
        };
        block.extend_from_slice(&notes);
        block.extend_from_slice(b";\n");
        let metadata = ChartData {
            max_player_index: players - 1,
            ..Default::default()
        };
        session.insert_empty_chart(block, &grid, metadata)
    }
}
fn starting_signature(document: &Document) -> Result<Signature, Error> {
    let mut groups: Vec<(&str, Vec<Signature>)> = Vec::new();
    for (index, chart) in document.charts().iter().enumerate() {
        if layout_columns(&chart.steps_type).is_none() {
            continue;
        }
        let signature = Signatures::from_document(document, index)?.at(0);
        if let Some((_, values)) = groups
            .iter_mut()
            .find(|(kind, _)| *kind == chart.steps_type)
        {
            values.push(signature);
        } else {
            groups.push((&chart.steps_type, vec![signature]));
        }
    }
    let mut histogram: Vec<(Signature, usize)> = Vec::new();
    for signature in groups.into_iter().flat_map(|(_, values)| values) {
        // Fumen Fraction's dictionary contract checks its written-ratio hash
        // before rational equality. Preserve the first representative key.
        let hash = |s: Signature| s.numerator.rotate_left(2) ^ s.denominator;
        if let Some((_, count)) = histogram.iter_mut().find(|(other, _)| {
            hash(*other) == hash(signature)
                && u64::from(other.numerator) * u64::from(signature.denominator)
                    == u64::from(signature.numerator) * u64::from(other.denominator)
        }) {
            *count += 1;
        } else {
            histogram.push((signature, 1));
        }
    }
    Ok(histogram
        .iter()
        .reduce(|a, b| if a.1 > b.1 { a } else { b })
        .map_or(
            Signature {
                numerator: 4,
                denominator: 4,
            },
            |entry| entry.0,
        ))
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
