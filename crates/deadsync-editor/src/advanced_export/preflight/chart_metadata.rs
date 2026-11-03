//! Explicit review of SSC chart metadata scopes omitted by an SM target.
use super::*;
impl Plan {
    pub(super) fn inspect_chart_metadata(
        &mut self,
        document: &Document,
        chart: usize,
        limits: Limits,
    ) -> Result<(), Error> {
        if self.target==Format::Ssc && !self.options.anonymize {
            let local=crate::chart_text::local_value(document,chart,crate::chart_text::Field::Credit)?;
            let authored=crate::chart_text::Text::read(document,chart)?;
            if local.as_deref()==Some("") && authored.get(crate::chart_text::Field::Credit).is_empty()
                && document.text(None,"CREDIT").is_some_and(|value|!value.is_empty()) {
                self.add(Some(chart),Severity::Warning,IssueKind::OmittedChartMetadata,"CREDIT",None,
                    "Source SSC reload inherits song CREDIT when chart CREDIT is empty; the explicit empty authored value changes",limits)?;
            }
        }
        if self.target != Format::Sm {
            return Ok(());
        }
        for field in [
            "MUSIC",
            "CHARTNAME",
            "CHARTSTYLE",
            "CREDIT",
            "SAMPLESTART",
            "SAMPLELENGTH",
            "SELECTABLE",
        ] {
            // Presence matters even for an empty override: its independent scope
            // is no longer representable. Radar is retained in the SM header.
            let logical_field = crate::chart_text::Field::ALL
                .into_iter()
                .find(|candidate| candidate.tag() == field);
            let local = if let Some(field) = logical_field {
                crate::chart_text::local_value(document, chart, field)?
            } else {
                document.text(Some(chart), field)
            };
            let Some(local) = local else {
                continue;
            };
            if self.options.anonymize && matches!(field, "CHARTNAME" | "CHARTSTYLE" | "CREDIT") {
                continue;
            }
            let shared = document.text(None, field).unwrap_or_default();
            let same = if matches!(field, "SAMPLESTART" | "SAMPLELENGTH") {
                match (local.parse::<f64>(), shared.parse::<f64>()) {
                    (Ok(a), Ok(b)) if a.is_finite() && b.is_finite() => a == b,
                    _ => local == shared,
                }
            } else {
                local == shared
            };
            let detail = match field {
                "MUSIC" | "CREDIT" if local.is_empty() || same => {
                    "SM drops this independent chart scope; its effective song fallback remains the same"
                }
                "MUSIC" => {
                    "SM drops this chart music override and uses song MUSIC; the music played for this chart changes"
                }
                "CREDIT" => "SM drops this chart credit override and uses song CREDIT",
                "CHARTNAME" | "CHARTSTYLE" => {
                    "SM drops this chart metadata; there is no corresponding chart field in its NOTES header"
                }
                _ if same => {
                    "SM drops this chart metadata scope; the corresponding song value is equal"
                }
                _ => {
                    "SM drops this chart metadata scope and retains only the song value; chart-local playback support is not assumed"
                }
            };
            self.add(
                Some(chart),
                Severity::Warning,
                IssueKind::OmittedChartMetadata,
                field,
                None,
                detail,
                limits,
            )?;
        }
        if !self.options.anonymize {
            let description = crate::chart_text::Text::read(document, chart)?
                .get(crate::chart_text::Field::Description)
                .to_owned();
            if description.trim_matches(['\r', '\n', '\t', ' ']) != description {
                self.add(Some(chart),Severity::Warning,IssueKind::OmittedChartMetadata,"DESCRIPTION",None,
                    "SM reload trims chart description whitespace; the exact authored text is not representable",limits)?;
            }
        }
        Ok(())
    }
}
