//! GrooveAuthor Editor*Config.ToString, independent of persisted profile bytes.
use crate::generation::{config::*,profiles::Config};
use crate::Error;
pub fn describe(name:&str,config:&Config,fixed2:&mut impl FnMut(f64)->Result<String,Error>)->Result<String,Error>{
    let abbreviation=match config {
        Config::Expressed(_)=>String::new(),
        Config::Pattern(c)=>{
            let start=|choice:StartFootChoice,lane:i32|match choice{StartFootChoice::SpecifiedLane=>lane.to_string(),StartFootChoice::AutomaticNewLane=>"N".into(),StartFootChoice::AutomaticSameLane=>"S".into(),StartFootChoice::AutomaticSameOrNewLane=>"A".into()};
            let end=|choice:EndFootChoice,lane:i32|match choice{EndFootChoice::SpecifiedLane=>lane.to_string(),EndFootChoice::AutomaticIgnoreFollowingSteps=>"I".into(),EndFootChoice::AutomaticNewLaneToFollowing=>"N".into(),EndFootChoice::AutomaticSameLaneToFollowing=>"S".into(),EndFootChoice::AutomaticSameOrNewLaneAsFollowing=>"A".into()};
            let repetition=if c.limit_same_arrows_in_a_row_per_foot{format!(" {}",c.max_same_arrows_in_a_row_per_foot)}else{String::new()};
            let period=if c.step_type_check_period>1{format!("x{}",c.step_type_check_period)}else{String::new()};
            let foot=match c.starting_foot_choice{StartingFootChoice::Specified=>if c.starting_foot_specified==0{"L"}else{"R"},StartingFootChoice::Automatic=>"A",StartingFootChoice::Random=>"?"};
            format!("1/{}{} {}/{}{} {} [{}|{}]->[{}|{}]",4*c.beat_sub_division,repetition,c.same_arrow_step_weight,c.new_arrow_step_weight,period,foot,start(c.left_foot_start_choice,c.left_foot_start_lane_specified),start(c.right_foot_start_choice,c.right_foot_start_lane_specified),end(c.left_foot_end_choice,c.left_foot_end_lane_specified),end(c.right_foot_end_choice,c.right_foot_end_lane_specified))
        },
        Config::Performed(c)=>{
            let mut parts=Vec::new();
            if c.step_tightening.distance_tightening_enabled==Some(true){parts.push(format!("{}S",fixed2(c.step_tightening.distance_min)?));}
            if c.transitions.enabled==Some(true){parts.push(format!("{}T",c.transitions.steps_per_transition_min));}
            if c.facing.max_inward_percentage<1.0{parts.push(format!("{}%",(c.facing.max_inward_percentage*100.0) as i32));}
            parts.join(" ")
        }
    };
    Ok(if name.is_empty(){abbreviation}else if abbreviation.is_empty(){name.to_owned()}else{format!("{name}: {abbreviation}")})
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn pattern_summary_covers_all_source_abbreviation_sections(){
  let c=PatternConfig{beat_sub_division:4,limit_same_arrows_in_a_row_per_foot:true,max_same_arrows_in_a_row_per_foot:3,same_arrow_step_weight:2,new_arrow_step_weight:9,step_type_check_period:4,starting_foot_choice:StartingFootChoice::Specified,starting_foot_specified:1,left_foot_start_choice:StartFootChoice::SpecifiedLane,left_foot_start_lane_specified:7,right_foot_start_choice:StartFootChoice::AutomaticNewLane,left_foot_end_choice:EndFootChoice::AutomaticSameLaneToFollowing,right_foot_end_choice:EndFootChoice::AutomaticSameOrNewLaneAsFollowing,..Default::default()};
  assert_eq!(describe("  named  ",&Config::Pattern(c),&mut |_|unreachable!()).unwrap(),serde_json::from_str::<serde_json::Value>(include_str!("../../../tests/fixtures/profile-presentation/expected.json")).unwrap()[0]["pattern"].as_str().unwrap());
 }
 #[test] fn performed_summary_preserves_source_enable_flags_and_cultural_formatter(){
  let mut c=PerformedConfig::default();c.facing.max_inward_percentage=0.429;c.step_tightening.distance_min=1.25;c.step_tightening.distance_tightening_enabled=Some(true);c.transitions.enabled=Some(true);c.transitions.steps_per_transition_min=15;
  assert_eq!(describe("",&Config::Performed(c.clone()),&mut |_|Ok("1,25".into())).unwrap(),serde_json::from_str::<serde_json::Value>(include_str!("../../../tests/fixtures/profile-presentation/expected.json")).unwrap()[1]["performed"].as_str().unwrap());
  c.step_tightening.distance_tightening_enabled=None;c.transitions.enabled=None;c.facing.max_inward_percentage=1.0;
  assert_eq!(describe("only name",&Config::Performed(c),&mut |_|unreachable!()).unwrap(),"only name");
  assert_eq!(describe("",&Config::Expressed(ExpressedConfig::default()),&mut |_|unreachable!()).unwrap(),"");
 }
}
