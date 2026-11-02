use deadsync_editor::options_preferences::{General,dpi::Capture};
use serde_json::Value;
#[test]
fn ten_original_dpi_decisions_and_lifetime_cache_are_preserved(){
 let cases:Value=serde_json::from_str(include_str!("fixtures/dpi.expected.json")).unwrap();for case in cases.as_array().unwrap(){let mut p=General::default();p.custom_dpi=case["Custom"].as_bool().unwrap();p.dpi_scale=case["Scale"].as_f64().unwrap();let captured=Capture::new(&p,case["Monitor"].as_f64().unwrap());assert_eq!(captured.scale(),case["First"].as_f64().unwrap());assert_eq!(captured.ui_scaled(2).unwrap(),case["ScaledTwo"].as_i64().unwrap()as i32);assert_eq!(captured.ui_scaled(18).unwrap(),case["CloseWidth"].as_i64().unwrap()as i32);assert_eq!(captured.font_pixels().unwrap(),case["FontPixels"].as_i64().unwrap()as i32);p.custom_dpi=true;p.dpi_scale=3.;assert_eq!(captured.scale(),case["AfterPreferenceAndMonitorChange"].as_f64().unwrap());assert_eq!(captured.pending_restart(&p),captured.scale()!=3.);}
}
#[test]
fn pages_are_bounded_keep_every_selected_row_visible_and_do_not_truncate(){
 use deadsync_editor::options_preferences::dpi::Page;
 let centers:Vec<_>=(0..5001).map(|i|100.+i as f32*32.).collect();for selected in [0,1,17,4096,5000]{let page=Page::rows(&centers,selected,32.,90.).unwrap();assert!(page.first<=selected&&selected<page.end);assert_eq!(page.end-page.first,1);let y=centers[selected]+page.translation;assert!(y>=16.&&y<=74.);assert_eq!(page.count,5001);}
 let page=Page::rows(&[30.,60.,90.],1,30.,480.).unwrap();assert_eq!(page.translation,0.);assert_eq!((page.first,page.end,page.count),(0,3,1));assert!(Page::rows(&[f32::NAN],0,30.,90.).is_none());
}

#[test]fn native_widget_baseline_preserves_dpi_one_and_uses_executed_native_style_font_rounding(){
 let cases:Value=serde_json::from_str(include_str!("fixtures/dpi-imgui.expected.json")).unwrap();for case in cases.as_array().unwrap(){let mut p=General::default();p.custom_dpi=true;p.dpi_scale=case["Scale"].as_f64().unwrap();let c=Capture::new(&p,1.);let metrics=c.native_widget_metrics(80.,20.);if case["Loaded"]==false{assert!(metrics.is_err());continue;}let m=metrics.unwrap();assert_eq!(m.height,20.*case["WidgetHeight"].as_f64().unwrap()/23.);assert_eq!(m.font_scale,case["FontSize"].as_f64().unwrap()/15.);if p.dpi_scale==1.{assert_eq!((m.width,m.height,m.padding),(80.,20.,2.));}}
}
