use deadsync_editor::{document::{Document,Format},history::Session,generation::{profile_catalog::{Task,Context},profiles::Kind}};
use std::{sync::{Arc,atomic::AtomicBool},path::Path};
#[test]
fn application_catalog_is_chartless_and_cannot_be_used_as_a_chart_selection(){
 let session=Session::new(Document::parse(Arc::<[u8]>::from(&b"#VERSION:0.83;#BPMS:0=120;"[..]),Format::Ssc).unwrap());
 assert!(session.document().charts().is_empty());assert!(Task::capture(&session,0,None).is_err());
 let catalog=Task::capture_application(&session,None::<&Path>).unwrap().run(&AtomicBool::new(false)).unwrap();
 assert!(catalog.is_current_in(&session,Context::Application));assert!(!catalog.is_current(&session,0));
 for kind in [Kind::Pattern,Kind::Performed,Kind::Expressed]{let id=&catalog.entries(kind)[0].guid;assert!(catalog.profile_in(&session,Context::Application,kind,id).is_ok());assert!(catalog.profile(&session,0,kind,id).is_err());}
 let foreign=Session::new(Document::parse(Arc::<[u8]>::from(&b"#VERSION:0.83;#BPMS:0=120;"[..]),Format::Ssc).unwrap());assert!(!catalog.is_current_in(&foreign,Context::Application));
}
