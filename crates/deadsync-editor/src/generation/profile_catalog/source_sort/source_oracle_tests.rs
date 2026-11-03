//! Differential fixtures executed from the unchanged pinned .NET comparer classes.
use super::*;
use crate::{
    document::{Document, Format},
    generation::profiles::Profile,
    history::Session,
};
use serde_json::Value as Json;
fn column(name: &str) -> Column {
    match name {
        "Name" => Column::Name,
        "Abbreviation" => Column::Abbreviation,
        "NoteType" => Column::NoteType,
        "RepetitionLimit" => Column::RepetitionLimit,
        "StepType" => Column::StepType,
        "StepTypeCheckPeriod" => Column::StepTypeCheckPeriod,
        "StartingFoot" => Column::StartingFoot,
        "StartingFooting" => Column::StartingFooting,
        "EndingFooting" => Column::EndingFooting,
        "StepSpeedMin" => Column::StepSpeedMin,
        "StepDistanceMin" => Column::StepDistanceMin,
        "StepStretchMin" => Column::StepStretchMin,
        "LateralSpeed" => Column::LateralSpeed,
        "LateralRelativeNPS" => Column::LateralRelativeNps,
        "LateralAbsoluteNPS" => Column::LateralAbsoluteNps,
        "TransitionMin" => Column::TransitionMin,
        "TransitionMax" => Column::TransitionMax,
        "FacingInwardLimit" => Column::FacingInwardLimit,
        "FacingOutwardLimit" => Column::FacingOutwardLimit,
        _ => panic!("unknown original source column {name}"),
    }
}
fn models(kind: Kind, family: &Json) -> Vec<(Entry, Option<Profile>)> {
    family["entries"].as_array().unwrap().iter().map(|row|{
 let config=match kind{Kind::Pattern=>Config::Pattern(serde_json::from_value(row["config"].clone()).unwrap()),Kind::Performed=>Config::Performed(serde_json::from_value(row["config"].clone()).unwrap()),Kind::Expressed=>unreachable!()};
 let profile=if kind==Kind::Performed {Some(Profile::parse(kind,&serde_json::to_vec(&serde_json::json!({"Guid":row["id"],"Name":row["name"],"Config":row["config"],"TravelSpeedMinBPM":row["travelSpeedMinBpm"],"TravelSpeedMaxBPM":500,"TravelSpeedNoteTypeDenominatorIndex":3})).unwrap()).unwrap())}else{None};
 (Entry{guid:row["id"].as_str().unwrap().parse().unwrap(),name:row["name"].as_str().map(str::to_owned),builtin:None,config,display:Some(row["display"].as_str().unwrap().into())},profile)
 }).collect()
}
fn catalog(session: &Session, kind: Kind, models: &[(Entry, Option<Profile>)]) -> Catalog {
    Catalog {
        token: session.revision_token(),
        context: crate::generation::profile_catalog::Context::Chart(0),
        entries: std::collections::BTreeMap::from([(
            kind,
            models.iter().map(|(entry, _)| entry.clone()).collect(),
        )]),
        dependencies: Default::default(),
        custom_profiles: models
            .iter()
            .filter_map(|(entry, p)| p.clone().map(|p| ((kind, entry.guid.clone()), p)))
            .collect(),
        issues: Vec::new(),
    }
}
fn ids(c: &Catalog, kind: Kind) -> Vec<String> {
    c.entries(kind)
        .iter()
        .map(|entry| entry.guid.to_string())
        .collect()
}
fn source_ids(row: &Json) -> Vec<String> {
    row.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}
#[test]
fn all_21_original_columns_1512_pair_comparisons_both_directions_and_multispecs() {
    let oracle: Json = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/profile-comparers/source.expected.json"
    ))
    .unwrap();
    assert_eq!(
        oracle["sourcePin"],
        "48e18f71655d0f0d86144c63239331833501fda2"
    );
    assert_eq!(oracle["pairComparisons"], 1512);
    let session = Session::new(
        Document::parse(
            b"#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;".as_slice(),
            Format::Ssc,
        )
        .unwrap(),
    );
    let bytes = session.serialize().unwrap();
    let revision = session.revision();
    let mut count = 0;
    for (kind, name) in [(Kind::Pattern, "pattern"), (Kind::Performed, "performed")] {
        let family = &oracle[name];
        let models = models(kind, family);
        assert_eq!(
            family["columns"].as_array().unwrap().len(),
            columns(kind).len()
        );
        for source_column in family["columns"].as_array().unwrap() {
            let selected = column(source_column["name"].as_str().unwrap());
            for trial in source_column["trials"].as_array().unwrap() {
                let specs = [Spec {
                    column: selected,
                    descending: trial["descending"].as_bool().unwrap(),
                }];
                let mut actual = catalog(&session, kind, &models);
                actual.apply_source_sort(kind, &specs).unwrap();
                assert_eq!(
                    ids(&actual, kind),
                    source_ids(&trial["order"]),
                    "{name} {selected:?} {:?}",
                    specs
                );
                for a in 0..models.len() {
                    for b in 0..models.len() {
                        let sign = trial["comparisons"][a][b].as_i64().unwrap();
                        let pair = vec![models[a].clone(), models[b].clone()];
                        let mut actual = catalog(&session, kind, &pair);
                        actual.apply_source_sort(kind, &specs).unwrap();
                        let expected = if sign > 0 {
                            vec![models[b].0.guid.to_string(), models[a].0.guid.to_string()]
                        } else {
                            vec![models[a].0.guid.to_string(), models[b].0.guid.to_string()]
                        };
                        assert_eq!(
                            ids(&actual, kind),
                            expected,
                            "{name} {selected:?} descending={} pair{a},{b} originalSign{sign}",
                            specs[0].descending
                        );
                        count += 1;
                    }
                }
            }
        }
        let specs = family["multiple"]["specs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| Spec {
                column: column(s["column"].as_str().unwrap()),
                descending: s["descending"].as_bool().unwrap(),
            })
            .collect::<Vec<_>>();
        let mut actual = catalog(&session, kind, &models);
        actual.apply_source_sort(kind, &specs).unwrap();
        assert_eq!(ids(&actual, kind), source_ids(&family["multiple"]["order"]));
    }
    assert_eq!(count, 1512);
    assert_eq!(session.serialize().unwrap(), bytes);
    assert_eq!(session.revision(), revision);
    assert!(session.undo_label().is_none());
}
