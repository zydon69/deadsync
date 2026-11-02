use deadsync_editor::document::{Document,Format,metadata::Metadata};
use serde_json::Value;
#[test]
fn read_only_preview_and_editable_document_use_the_same_literal_schema() {
 let cases:Vec<Value>=serde_json::from_str(include_str!("fixtures/key_commands/text-whitespace-source.json")).unwrap();
 for case in cases {
  let format=if case["format"]=="sm" {Format::Sm}else{Format::Ssc};
  let input=case["input"].as_str().unwrap().as_bytes().to_vec();
  let editable=Document::parse(input.clone(),format).unwrap();let preview=Metadata::parse(input,format).unwrap();
  for tag in ["TITLE","BANNER","CREDIT","JACKET","BPMS"] { assert_eq!(preview.text(tag),editable.text(None,tag),"{case} {tag}"); }
 }
}
#[test]
fn read_only_chart_description_retains_ssc_literal_but_normalizes_sm_header() {
 let cases:Vec<Value>=serde_json::from_str(include_str!("fixtures/key_commands/chart-text-whitespace-source.json")).unwrap();
 for case in cases {
  let format=if case["format"]=="sm" {Format::Sm}else{Format::Ssc};
  let input=case["input"].as_str().unwrap().as_bytes().to_vec();
  let preview=Metadata::parse(input,format).unwrap();
  assert_eq!(preview.charts()[0].description,case["description"].as_str().unwrap());
 }
}
