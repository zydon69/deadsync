//! Prepare the song-reference half of GrooveAuthor profile deletion.
//! All charts are validated before one undoable edit is committed.
use super::{
    metadata::{self, ChartData, Guid, Registry},
    profiles::Kind,
};
use crate::{
    Error,
    history::{RevisionToken, Session},
};
use std::sync::Arc;

pub struct Reset {
    token: RevisionToken,
    properties: Vec<(Option<usize>, String, Arc<[u8]>)>,
    references: usize,
}
impl Reset {
    pub fn capture(session: &Session, kind: Kind, source: &Guid) -> Result<Self, Error> {
        let registry = Registry::builtin();
        let protected = match kind {
            Kind::Pattern => registry.patterns.contains_key(source),
            Kind::Performed => registry.performed.contains_key(source),
            Kind::Expressed => registry.expressed.contains_key(source),
        };
        if protected {
            return Err(Error::InvalidEdit(
                "Built-in generation profiles cannot be deleted".into(),
            ));
        }
        // Original ActionDelete*Config fallback GUIDs, including the general
        // movement default (not the pattern-specific movement preset).
        let fallback: Guid = match kind {
            Kind::Pattern => metadata::SIXTEENTHS,
            Kind::Performed => "6276c906-ea8f-43b3-9500-0ddeac7bdc22",
            Kind::Expressed => metadata::DYNAMIC,
        }
        .parse()?;
        let mut properties = Vec::new();
        let mut references = 0;
        for chart in 0..session.document().charts().len() {
            let Some(mut data) = ChartData::read(session.document(), chart)? else {
                continue;
            };
            let mut count = 0;
            if kind == Kind::Expressed {
                if data.expressed_chart_config == *source {
                    data.expressed_chart_config = fallback.clone();
                    count += 1;
                }
            } else if let Some(patterns) = data.patterns.as_mut() {
                for marker in patterns.values_mut() {
                    let id = if kind == Kind::Pattern {
                        &mut marker.pattern_config_guid
                    } else {
                        &mut marker.performed_chart_config_guid
                    };
                    if *id == *source {
                        *id = fallback.clone();
                        count += 1;
                    }
                }
            }
            if count == 0 {
                continue;
            }
            data.validate()?;
            let (scope, _, name) = metadata::tags(session.document(), chart)?;
            let json =
                serde_json::to_string(&data).map_err(|e| Error::InvalidFormat(e.to_string()))?;
            properties.push((scope, name, crate::document::escape_text(&json).into()));
            references += count;
        }
        Ok(Self {
            token: session.revision_token(),
            properties,
            references,
        })
    }
    pub fn affected_charts(&self) -> usize {
        self.properties.len()
    }
    pub fn affected_references(&self) -> usize {
        self.references
    }
    pub fn apply(self, session: &mut Session) -> Result<bool, Error> {
        if !session.accepts_token(&self.token) {
            return Err(Error::InvalidEdit(
                "Profile references belong to another document or revision".into(),
            ));
        }
        session.set_property_bytes_atomic(
            &self.properties,
            "Reset deleted generation profile references",
        )
    }
}
