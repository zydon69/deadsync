//! Typed StepF2 inspector state. The caller retains the single shared Draft and
//! text buffer used by both typed and raw inspector presentations.
use deadsync_editor::{
    Error,
    note_attributes::{Draft, Field},
    stepf2::{Attributes, Visibility},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Row {
    Lane,
    Endpoint,
    Visibility,
    Fake,
    Reserved,
    Raw,
    Apply,
    Cancel,
}

impl Row {
    pub(super) const ALL: [Self; 8] = [
        Self::Lane,
        Self::Endpoint,
        Self::Visibility,
        Self::Fake,
        Self::Reserved,
        Self::Raw,
        Self::Apply,
        Self::Cancel,
    ];
}

pub(super) struct Inspector {
    endpoint: Field,
}

impl Default for Inspector {
    fn default() -> Self {
        Self {
            endpoint: Field::Head,
        }
    }
}

impl Inspector {
    pub(super) fn available(draft: &Draft) -> bool {
        // The raw view must remain usable if a currently incomplete compound
        // cannot be parsed. Entry to typed mode separately calls attributes().
        matches!(draft.stepf2_attributes(Field::Head), Ok(Some(_)))
    }

    pub(super) fn endpoint(&self) -> Field {
        self.endpoint
    }

    pub(super) fn select_endpoint(&mut self, draft: &Draft, endpoint: Field) -> Result<(), Error> {
        draft.stepf2_attributes(endpoint)?.ok_or_else(|| {
            Error::InvalidEdit("StepF2 attributes are unavailable for this endpoint".into())
        })?;
        self.endpoint = endpoint;
        Ok(())
    }

    pub(super) fn attributes(&self, draft: &Draft) -> Result<Attributes, Error> {
        draft.stepf2_attributes(self.endpoint)?.ok_or_else(|| {
            Error::InvalidEdit("StepF2 attributes are unavailable for this endpoint".into())
        })
    }

    /// Called only after the caller synchronizes pending text. Parsing happens
    /// before publishing an endpoint change, so malformed input remains focused.
    pub(super) fn toggle_endpoint(&mut self, draft: &Draft) -> Result<bool, Error> {
        self.attributes(draft)?;
        if draft.note().end.is_none() {
            return Ok(false);
        }
        let next = match self.endpoint {
            Field::Head => Field::Tail,
            Field::Tail => Field::Head,
        };
        draft.stepf2_attributes(next)?.ok_or_else(|| {
            Error::InvalidEdit("StepF2 attributes are unavailable for this endpoint".into())
        })?;
        self.endpoint = next;
        Ok(true)
    }

    pub(super) fn cycle_visibility(&self, draft: &mut Draft, right: bool) -> Result<(), Error> {
        let mut attributes = self.attributes(draft)?;
        let all = [
            Visibility::Normal,
            Visibility::Sudden,
            Visibility::Vanish,
            Visibility::Hidden,
        ];
        let current = all
            .iter()
            .position(|&v| v == attributes.visibility)
            .unwrap();
        attributes.visibility = all[(current + if right { 1 } else { 3 }) % all.len()];
        draft.set_stepf2_attributes(self.endpoint, attributes)
    }

    pub(super) fn toggle_fake(&self, draft: &mut Draft) -> Result<(), Error> {
        let mut attributes = self.attributes(draft)?;
        attributes.fake = !attributes.fake;
        draft.set_stepf2_attributes(self.endpoint, attributes)
    }

    pub(super) fn set_reserved(&self, draft: &mut Draft, text: &str) -> Result<(), Error> {
        let mut characters = text.chars();
        let reserved = characters
            .next()
            .filter(|_| characters.next().is_none())
            .ok_or_else(|| {
                Error::InvalidEdit("StepF2 reserved field requires exactly one character".into())
            })?;
        let mut attributes = self.attributes(draft)?;
        attributes.reserved = reserved;
        // The domain codec validates the complete character set before it
        // mutates the shared draft; no UI-only validation rules are duplicated.
        draft.set_stepf2_attributes(self.endpoint, attributes)
    }
}
