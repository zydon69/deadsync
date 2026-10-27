//! Mount adapter for the simplified native layout. Vertical navigation remains
//! in the previously validated native viewport; only horizontal mounting changes.
use super::*;
use deadsync_editor::minimap_mount::{Bounds, FocusEdges, Input, Padding, Rect};
use deadsync_editor::minimap_position_preferences::Preferences;
impl State {
    pub fn minimap_position_preferences(&self) -> Preferences {
        self.minimap.position
    }
    pub fn configure_minimap_position_preferences(&mut self, preferences: Preferences) {
        self.minimap_release_input();
        self.minimap.position = preferences;
    }
    pub(super) fn focused_minimap_mount_layout(
        &self,
    ) -> Option<super::minimap_mount_layout::Layout> {
        let geometry = self
            .visible_view_geometry()
            .into_iter()
            .find(|view| view.focused)?;
        super::minimap_mount_layout::layout(super::minimap_mount_layout::Input {
            left: geometry.left,
            lane_width: geometry.lane_width,
            columns: self
                .preview
                .as_ref()
                .map(GenerationPreview::grid)
                .unwrap_or_else(|| self.workspace.grid())
                .columns(),
            size_zoom: self.zoom.size_zoom() as f32,
            clip_right: geometry.clip_right,
            misc_width: 96.0,
            // Same source-family gate as the focused participant actor. The
            // renderer uses the document layout even during generation preview.
            multiplayer: super::player_indicator::source_multiplayer(
                &self.workspace.file().session.document().charts()[self.workspace.chart()]
                    .steps_type,
            ),
        })
    }
    pub fn minimap_mount_bounds(&self) -> Result<Bounds, deadsync_editor::Error> {
        let focus = self
            .focused_minimap_mount_layout()
            .map(|layout| FocusEdges {
                without_scale: layout.without_scale as i32,
                with_scale: layout.with_scale as i32,
            });
        let height = screen_height().max(0.0) as i32;
        deadsync_editor::minimap_mount::bounds(
            self.minimap.position,
            self.minimap.content.options.width,
            Input {
                chart_area: Rect {
                    x: 0,
                    y: 0,
                    width: screen_width().max(0.0) as i32,
                    height,
                },
                viewport_height: height,
                focus,
                // Native insets intentionally preserve the validated y96 projection.
                padding: Padding {
                    top_screen: 96,
                    top_chart: 96,
                    bottom: 96,
                },
                dpi_scale: 1.0,
            },
        )
    }
}
