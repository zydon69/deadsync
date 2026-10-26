// Existing native tests use ASCII chart labels. Complete the same deferred
// boundary as the shell without opening a system ICU handle in these tests.
fn complete_chart_navigation(state: &mut deadsync_theme_simply_love::screens::edition::State) {
    struct TestOrderer;
    impl deadsync_editor::chart_order::Orderer for TestOrderer {
        fn order(
            &mut self,
            snapshot: &deadsync_editor::chart_order::Snapshot,
        ) -> Result<Vec<usize>, deadsync_editor::Error> {
            snapshot.indices_with(str::cmp)
        }
    }
    state.process_chart_order(&mut TestOrderer);
}
