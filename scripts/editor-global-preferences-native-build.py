from pathlib import Path
root=Path('/Users/zydon/Sites/feetness/deadsync');base=Path('/tmp/deadsync-source-preferences/base');work=base.parent/'work'
def copy(rel):
 p=Path(rel);src=(root/p).read_text();(base/p).parent.mkdir(parents=True,exist_ok=True);(work/p).parent.mkdir(parents=True,exist_ok=True);(base/p).write_text(src);return p,src
rel,s=copy('crates/deadsync-theme-simply-love/src/screens/edition.rs')
s=s.replace('mod general_options;','mod general_options;\nmod source_preferences;')
s=s.replace('    GeneralOptions(usize),','    GeneralOptions(usize),\n    SourcePreferencesImport(usize),\n    SourcePreferencesReview(usize),')
s=s.replace('    general_field:usize,','    general_field:usize,\n    source_preferences_import:source_preferences::ImportUi,')
s=s.replace('general_options:Default::default(),general_field:0,','general_options:Default::default(),general_field:0,source_preferences_import:Default::default(),')
s=s.replace('        if let Panel::GeneralOptions(index)=panel{','        if let Panel::SourcePreferencesImport(index)|Panel::SourcePreferencesReview(index)=panel{self.source_preferences_key(index,code);return false;}\n        if let Panel::GeneralOptions(index)=panel{')
s=s.replace('            Panel::GeneralOptions(_)=>unreachable!', '            Panel::SourcePreferencesImport(_)|Panel::SourcePreferencesReview(_)=>unreachable!("Source preferences dispatched above"),\n            Panel::GeneralOptions(_)=>unreachable!')
s=s.replace('            Panel::GeneralOptions(_)=> &general_options::GENERAL_KEYS,','            Panel::SourcePreferencesImport(_)=> &source_preferences::IMPORT_KEYS,\n            Panel::SourcePreferencesReview(_)=> &source_preferences::REVIEW_KEYS,\n            Panel::GeneralOptions(_)=> &general_options::GENERAL_KEYS,')
s=s.replace('            | Panel::GeneralOptions(i)','            | Panel::SourcePreferencesImport(i)\n            | Panel::SourcePreferencesReview(i)\n            | Panel::GeneralOptions(i)')
s=s.replace('            } else if matches!(panel,Panel::GeneralOptions(_)){','            } else if matches!(panel,Panel::SourcePreferencesImport(_)|Panel::SourcePreferencesReview(_)){self.source_preferences_label(index,key)\n            } else if matches!(panel,Panel::GeneralOptions(_)){')
s=s.replace('        let before_revision = self.workspace.file().session.revision();','        if matches!(command,Command::Undo|Command::Redo){match self.capture_application_preferences_file(){Ok(file)=>self.workspace.capture_source_preferences_context(&file),Err(error)=>{self.status=i18n::editor_error(&error).into();return;}}}\n        let before_revision = self.workspace.file().session.revision();',1)
s=s.replace('                self.sync_audio_preferences_history();','                if let Err(error)=self.sync_source_preferences_history(){self.status=i18n::editor_error(&error).into();self.refresh();return;}\n                self.sync_audio_preferences_history();',1)
(work/rel).write_text(s)
rel,s=copy('crates/deadsync-theme-simply-love/src/screens/edition/general_options.rs')
s=s.replace('GENERAL_KEYS: [&str; 4]','GENERAL_KEYS: [&str; 5]').replace('"GeneralAllDefaults"];','"GeneralAllDefaults", "SourcePreferencesOpen"];')
s=s.replace('        if self.startup_graph_key(index, code) {','        if index==4&&enter{self.open_source_preferences_import();return;}\n        if self.startup_graph_key(index, code) {')
s=s.replace('(index + if back { 3 } else { 1 }) % 4','(index + if back { 4 } else { 1 }) % 5')
# Import entry must not fall through the editable field logic.
s=s.replace('        if index == 3 {','        if index == 4{return;}\n        if index == 3 {')
(work/rel).write_text(s)
rel,s=copy('crates/deadsync-theme-simply-love/src/screens/edition/text_fields.rs')
s=s.replace('    FileOpenPath,','    FileOpenPath,\n    SourcePreferencesPath,',1).replace('            Self::FileOpenPath=>','            Self::SourcePreferencesPath=>"SourcePreferencesPath",\n            Self::FileOpenPath=>',1)
s=s.replace('            Some(Panel::FileOpen(_)) if index==0=>','            Some(Panel::SourcePreferencesImport(_)) if index==0=>Some(Target::SourcePreferencesPath),\n            Some(Panel::FileOpen(_)) if index==0=>',1)
s=s.replace('            Target::FileOpenPath=>Some(', '            Target::SourcePreferencesPath=>Some(self.source_preferences_import.path.as_str().into()),\n            Target::FileOpenPath=>Some(',1)
s=s.replace('            Target::FileOpenPath=>matches!', '            Target::SourcePreferencesPath=>matches!(self.panel,Some(Panel::SourcePreferencesImport(_))),\n            Target::FileOpenPath=>matches!',1)
s=s.replace('            Target::FileOpenPath=>{','''            Target::SourcePreferencesPath=>{
                if value.len()>4096||value.chars().any(char::is_control){return Err(deadsync_editor::Error::InvalidEdit("Invalid preference import path text".into()));}
                self.source_preferences_import.path=value.to_owned();self.source_preferences_import.draft=None;Ok(())
            }
            Target::FileOpenPath=>{''',1)
s=s.replace('Some(Panel::FileOpen(i) |','Some(Panel::SourcePreferencesImport(i) | Panel::FileOpen(i) |',1)
(work/rel).write_text(s)
p=work/'crates/deadsync-theme-simply-love/src/screens/edition/source_preferences.rs';p.write_text((root/'patches/editor-global-preferences-native.rs').read_text())
