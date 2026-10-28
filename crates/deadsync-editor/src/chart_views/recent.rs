//! Construct a freshly loaded registry from source descriptors, preserving the
//! focused fractional row even when multiple rows share a warped music time.
use super::*;
impl ActiveChartViews {
    pub fn restore_recent(
        session: &Session,
        restoration: &crate::recent_songs::Restoration,
        row: f64,
    ) -> Result<Self, Error> {
        if !row.is_finite() {
            return Err(Error::InvalidEdit("Invalid recent chart row".into()));
        }
        let mut views = Self::empty(session, 0.0)?;
        for &chart in &restoration.active {
            views.show(session, chart, true)?;
        }
        if let Some(chart) = restoration.focused {
            views.focus(session, chart)?;
            let position = views
                .focused()
                .ok_or_else(|| Error::InvalidEdit("Recent focused view is absent".into()))?
                .position_at_row(session, row)?;
            views.set_music_time(session, position.music_time())?;
            // set_music_time maps every other chart independently. Restore the
            // requested focused row after that shared-time operation, without
            // inverting the ambiguous time through a warp.
            let id = views
                .focused
                .clone()
                .ok_or_else(|| Error::InvalidEdit("Recent focused view is absent".into()))?;
            let entry = views
                .entries
                .iter_mut()
                .find(|entry| entry.identity == id)
                .ok_or_else(|| Error::InvalidEdit("Recent focused view is absent".into()))?;
            entry.position = position;
        } else if !restoration.active.is_empty() {
            return Err(Error::InvalidEdit(
                "Recent active views require a focused chart".into(),
            ));
        }
        Ok(views)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn session() -> Session {
        Session::new(crate::document::Document::parse(b"#VERSION:0.83;#BPMS:0=120;#WARPS:2=4;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-double;#BPMS:0=180;#NOTES:10000000;".as_slice(),crate::document::Format::Ssc).unwrap())
    }
    #[test]
    fn restored_views_keep_order_dedicated_tabs_and_exact_focused_warp_row() {
        let session = session();
        let expected = crate::chart_focus::Position::at_fractional_row(&session, 0, 160.5).unwrap();
        let mut views = ActiveChartViews::restore_recent(
            &session,
            &crate::recent_songs::Restoration {
                active: vec![1, 0, 1],
                focused: Some(0),
            },
            160.5,
        )
        .unwrap();
        assert_eq!(
            views
                .entries
                .iter()
                .map(ViewState::source_index)
                .collect::<Vec<_>>(),
            vec![1, 0]
        );
        assert!(views.entries.iter().all(ViewState::dedicated));
        assert_eq!(views.focused().unwrap().position().row(), 160.5);
        assert_eq!(views.music_time(), expected.music_time());
        views
            .synchronize(&session, Some(0), views.music_time())
            .unwrap();
        assert_eq!(views.focused().unwrap().position().row(), 160.5);
        assert!(
            views
                .entries()
                .iter()
                .all(|entry| entry.clock(&session).is_ok()
                    && entry.position().chart() == entry.source_index())
        );
    }
    #[test]
    fn fallback_focus_is_temporary_and_invalid_inputs_fail_without_touching_session() {
        let session = session();
        let original = session.serialize().unwrap();
        let views = ActiveChartViews::restore_recent(
            &session,
            &crate::recent_songs::Restoration {
                active: vec![],
                focused: Some(1),
            },
            -24.25,
        )
        .unwrap();
        assert!(!views.focused().unwrap().dedicated());
        assert_eq!(views.focused().unwrap().position().row(), -24.25);
        assert!(
            ActiveChartViews::restore_recent(
                &session,
                &crate::recent_songs::Restoration {
                    active: vec![99],
                    focused: Some(0)
                },
                0.0
            )
            .is_err()
        );
        assert!(
            ActiveChartViews::restore_recent(
                &session,
                &crate::recent_songs::Restoration {
                    active: vec![],
                    focused: None
                },
                f64::NAN
            )
            .is_err()
        );
        assert_eq!(session.serialize().unwrap(), original);
    }
}
