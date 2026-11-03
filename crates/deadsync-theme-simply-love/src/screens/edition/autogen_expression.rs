//! Source autogen Expression edits the active source chart. This does not change
//! the separately labelled readonly conversion override API.
use super::*;
impl State {
    pub(super) fn open_autogen_source_expression(&mut self,back:Panel){
        if self.is_generating(){return;}
        if self.chart_properties.is_some(){self.status=i18n::tr("Edition","AutogenSourceExpressionPending");return;}
        let source=if matches!(back,Panel::Generation(_)){
            self.autogen_source.showing=true;self.sync_autogen_source();self.autogen_source_chart()
        }else{self.workspace.active_chart()};
        let Some(source)=source else{self.status=self.autogen_source_label().into();return;};
        match deadsync_editor::chart_properties::Draft::capture(&self.workspace.file().session,source){
            Ok(draft)=>{
                self.autogen_source.binding=Some(deadsync_editor::chart_order::SessionBinding::capture(&self.workspace.file().session));
                self.autogen_source.expression_target=self.workspace.file().session.chart_ids().get(source).cloned();
                self.chart_properties=Some(draft);self.chart_rating_input=None;self.autogen_expression_back=Some(back);
                self.open_chart_expression();
                if self.catalog_job.is_none(){let status=self.status.clone();self.cancel_autogen_expression();self.status=status;}
            }
            Err(error)=>self.status=i18n::tr_fmt("Edition","Error",&[("message",&crate::i18n::editor_error(&error))]),
        }
    }
    pub(super) fn cancel_autogen_expression(&mut self)->bool{
        let Some(back)=self.autogen_expression_back.take() else{return false;};
        self.autogen_source.expression_target=None;
        self.chart_properties=None;self.chart_rating_input=None;self.profile_selection=None;self.panel=Some(back);
        self.status=i18n::tr("Edition","AutogenSourceExpressionCancelled");true
    }
    pub(super) fn autogen_source_expression_label(&self)->String{
        let chart=if matches!(self.panel,Some(Panel::Batch(_))){Some(self.workspace.chart())}else{self.autogen_source_chart()};
        let Some(chart)=chart else{return i18n::tr("Edition","ProfileMissing").to_string();};
        let id=match deadsync_editor::generation::metadata::ChartData::read(self.workspace.file().session.document(),chart){
            Ok(Some(data))=>data.expressed_chart_config,
            Ok(None)=>deadsync_editor::generation::metadata::DYNAMIC.parse().expect("constant GUID"),
            Err(_)=>return i18n::tr("Edition","ProfileMissing").to_string(),
        };
        let key=match id.to_string().as_str(){
            deadsync_editor::generation::metadata::DYNAMIC=>Some("ProfileDynamic"),
            "da3f6e12-49d1-416b-8db6-0ab413f740b6"=>Some("ProfileAggressiveBrackets"),
            "0c0ba200-8f90-4060-8912-e9ea65831ebc"=>Some("ProfileNoBrackets"),
            _=>None,
        };
        key.map_or_else(||id.to_string(),|key|i18n::tr("Edition",key).to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use deadsync_editor::generation::metadata::{ChartData,Guid,DYNAMIC};
    fn state(language:&str)->(tempfile::TempDir,State){
        crate::i18n::init_for_tests();crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        let root=tempfile::tempdir().unwrap();let path=root.path().join("song.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#TITLE:Source;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        (root,State::open(&path,0).unwrap())
    }
    fn pick(state:&mut State,id:&Guid){
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(2);
        while state.catalog_job.is_some(){state.update();assert!(std::time::Instant::now()<deadline,"profile catalog timeout");std::thread::yield_now();}
        assert!(matches!(state.panel,Some(Panel::ChartExpression(_))));
        let settings=state.profile_selection.as_mut().unwrap();settings.expressed=Some(settings.catalog.entries(ProfileKind::Expressed).iter().position(|entry|&entry.guid==id).unwrap());
    }
    #[test]
    fn source_control_changes_chart_properties_and_save_reload_undo_without_changing_readonly_override_en_fr(){
        for language in ["en","fr"]{
            let (root,mut state)=state(language);
            let readonly:Guid="da3f6e12-49d1-416b-8db6-0ab413f740b6".parse().unwrap();state.generation.profiles.expressed=Some(readonly.clone());
            let chosen:Guid="0c0ba200-8f90-4060-8912-e9ea65831ebc".parse().unwrap();
            state.open_autogen_source_expression(Panel::Generation(6));pick(&mut state,&chosen);state.stage_chart_expression();
            assert!(matches!(state.panel,Some(Panel::Generation(6))));assert!(state.chart_properties.is_none());
            assert_eq!(ChartData::read(state.workspace.file().session.document(),0).unwrap().unwrap().expressed_chart_config,chosen);
            assert_eq!(state.workspace.chart_properties().unwrap().expression(),&chosen);
            assert_eq!(state.generation.profiles.expressed,Some(readonly.clone()));
            state.command(Command::Save);
            let reopened=State::open(&root.path().join("song.ssc"),0).unwrap();assert_eq!(reopened.workspace.chart_properties().unwrap().expression(),&chosen);
            state.command(Command::Undo);assert_eq!(state.workspace.chart_properties().unwrap().expression().to_string(),DYNAMIC);
            assert_eq!(state.generation.profiles.expressed,Some(readonly));
            state.command(Command::Redo);assert_eq!(state.workspace.chart_properties().unwrap().expression(),&chosen);
        }
    }
    #[test]
    fn source_picker_cancel_and_stale_catalog_do_not_publish_chart_changes_en_fr(){
        for language in ["en","fr"]{
            let (_root,mut state)=state(language);let before=state.workspace.file().session.serialize().unwrap();
            let chosen:Guid="0c0ba200-8f90-4060-8912-e9ea65831ebc".parse().unwrap();
            state.open_autogen_source_expression(Panel::Batch(7));pick(&mut state,&chosen);
            assert!(state.cancel_autogen_expression());assert_eq!(state.workspace.file().session.serialize().unwrap(),before);assert!(matches!(state.panel,Some(Panel::Batch(7))));
            state.open_autogen_source_expression(Panel::Generation(6));pick(&mut state,&chosen);
            state.workspace.execute(Command::Place(1)).unwrap();let changed=state.workspace.file().session.serialize().unwrap();
            state.stage_chart_expression();assert_eq!(state.workspace.file().session.serialize().unwrap(),changed);assert!(state.autogen_expression_back.is_some());
            assert!(state.cancel_autogen_expression());assert!(state.chart_properties.is_none());
        }
    }
}
