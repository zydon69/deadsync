using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Nodes;
using ImGuiNET;
using StepManiaEditor;
using StepManiaEditor.AutogenConfig;
using StepManiaLibrary.PerformedChart;
class Program {
 static readonly string[] Ids={"ffffffff-0000-0000-0000-000000000001","80000000-0000-0000-0000-000000000002","7fffffff-ffff-0000-0000-000000000003","00000000-0000-0000-0000-000000000004","00000000-ffff-0000-0000-000000000005","00000000-0000-8000-0000-000000000006"};
 static readonly string[] Names={null,"","\uE000","\U00010000","é","z"};
 static int PairComparisons;
 static bool? Flag(int index)=>index%3==0?true:index%3==1?false:null;
 static EditorPatternConfig Pattern(int i){
  var c=new PatternConfig{BeatSubDivision=new[]{1,2,3,4,6,48}[i],SameArrowStepWeight=new[]{2,9,0,25,75,1}[i],NewArrowStepWeight=new[]{2,9,0,75,25,0}[i],StepTypeCheckPeriod=new[]{0,1,2,4,16,3}[i],LimitSameArrowsInARowPerFoot=i%2==0,MaxSameArrowsInARowPerFoot=new[]{3,9,1,2,4,0}[i],StartingFootChoice=(PatternConfigStartingFootChoice)(i%3),StartingFootSpecified=i%2,LeftFootStartChoice=(PatternConfigStartFootChoice)(i%4),RightFootStartChoice=(PatternConfigStartFootChoice)((i+1)%4),LeftFootStartLaneSpecified=i==0?10:i,RightFootStartLaneSpecified=i==1?10:2,LeftFootEndChoice=(PatternConfigEndFootChoice)(i%5),RightFootEndChoice=(PatternConfigEndFootChoice)((i+2)%5),LeftFootEndLaneSpecified=i==0?10:i,RightFootEndLaneSpecified=i==2?10:2};
  c.RefreshStepWeightsNormalized();var p=new EditorPatternConfig{Guid=Guid.Parse(Ids[i]),Name=Names[i],Config=c,PatternType=c.BeatSubDivision*4};p.Refresh();return p;
 }
 static EditorPerformedChartConfig Performed(int i){
  var c=new Config();c.ArrowWeights=new Dictionary<string,List<int>>();
  c.StepTightening.SpeedTighteningEnabled=i<2?true:Flag(i);c.StepTightening.SpeedMinTimeSeconds=0.01+i*0.02;c.StepTightening.SpeedMaxTimeSeconds=0.2+i*0.01;c.StepTightening.SpeedTighteningMinDistance=0.0;
  c.StepTightening.DistanceTighteningEnabled=Flag(i);c.StepTightening.DistanceMin=1.0+i*0.25;c.StepTightening.DistanceMax=3.0+i;
  c.StepTightening.StretchTighteningEnabled=Flag(i+1);c.StepTightening.StretchDistanceMin=0.5+i*0.1;c.StepTightening.StretchDistanceMax=3.0+i;
  c.StepTightening.LateralMinPanelDistance=0.0;c.StepTightening.LongitudinalMinPanelDistance=0.0;
  c.LateralTightening.Enabled=Flag(i+2);c.LateralTightening.Speed=1.0+i*0.5;c.LateralTightening.RelativeNPS=1.0+i*0.25;c.LateralTightening.AbsoluteNPS=2.0+i;
  c.Transitions.Enabled=Flag(i);c.Transitions.StepsPerTransitionMin=i*4;c.Transitions.StepsPerTransitionMax=32+i*16;c.Transitions.MinimumPadWidth=5;c.Transitions.TransitionCutoffPercentage=0.5;
  c.Facing.MaxInwardPercentage=i/5.0;c.Facing.InwardPercentageCutoff=0.34;c.Facing.MaxOutwardPercentage=(5-i)/5.0;c.Facing.OutwardPercentageCutoff=0.34;
  var p=new EditorPerformedChartConfig{Guid=Guid.Parse(Ids[i]),Name=Names[i],Config=c,TravelSpeedMinBPM=new[]{99,300,125,50,200,90}[i]};p.Refresh();return p;
 }
 static unsafe void Set(Action<ImGuiTableSortSpecsPtr> setter,(int column,bool descending)[] specs){
  var native=specs.Select(s=>new ImGuiUtils.NativeImGuiTableColumnSortSpecs{ColumnUserID=(uint)s.column,SortDirection=(byte)(s.descending?ImGuiSortDirection.Descending:ImGuiSortDirection.Ascending)}).ToArray();
  fixed(ImGuiUtils.NativeImGuiTableColumnSortSpecs* pointer=native){setter(new ImGuiTableSortSpecsPtr{Specs=new SpecsPointer{NativePtr=pointer},SpecsCount=native.Length});}
 }
 static object Matrix<T,TColumn>(T[] entries,IComparer<T> comparer,Action<ImGuiTableSortSpecsPtr> setter,Func<T,Guid> id,Func<T,object> serialize,string[] multiple,bool[] directions)where TColumn:struct,Enum {
  var names=Enum.GetNames<TColumn>().Where(name=>name!="Clone"&&name!="Delete").ToArray();var columns=new List<object>();
  foreach(var name in names){var trials=new List<object>();foreach(var descending in new[]{false,true}){
   Set(setter,new[]{(Convert.ToInt32(Enum.Parse<TColumn>(name)),descending)});
   var comparisons=entries.Select(a=>entries.Select(b=>{PairComparisons++;return Math.Sign(comparer.Compare(a,b));}).ToArray()).ToArray();
   var sorted=entries.ToList();sorted.Sort(comparer);
   trials.Add(new{descending,comparisons,order=sorted.Select(id).Select(g=>g.ToString()).ToArray()});
  }columns.Add(new{name,trials});}
  var specs=multiple.Select((name,i)=>(Convert.ToInt32(Enum.Parse<TColumn>(name)),directions[i])).ToArray();Set(setter,specs);var multi=entries.ToList();multi.Sort(comparer);
  return new{entries=entries.Select(serialize).ToArray(),columns,multiple=new{specs=multiple.Select((column,i)=>new{column,descending=directions[i]}).ToArray(),order=multi.Select(id).Select(g=>g.ToString()).ToArray()}};
 }
 static void Main(){CultureInfo.CurrentCulture=CultureInfo.InvariantCulture;CultureInfo.CurrentUICulture=CultureInfo.InvariantCulture;
  var patterns=Enumerable.Range(0,6).Select(Pattern).ToArray();var performed=Enumerable.Range(0,6).Select(Performed).ToArray();var pc=new UIPatternComparer();var fc=new UIPerformedChartComparer();
  var p=Matrix<EditorPatternConfig,UIPatternConfigTable.Column>(patterns,pc,pc.SetSortSpecs,x=>x.Guid,x=>new{id=x.Guid.ToString(),name=x.Name,config=JsonSerializer.SerializeToNode(x.Config),display=x.ToString()},new[]{"StepType","RepetitionLimit","Name"},new[]{false,true,true});
  var f=Matrix<EditorPerformedChartConfig,UIPerformedChartConfigTable.Column>(performed,fc,fc.SetSortSpecs,x=>x.Guid,x=>new{id=x.Guid.ToString(),name=x.Name,config=JsonSerializer.SerializeToNode(x.Config),display=x.ToString(),travelSpeedMinBpm=x.TravelSpeedMinBPM},new[]{"StepSpeedMin","StepDistanceMin","Name"},new[]{true,false,true});
  Console.WriteLine(JsonSerializer.Serialize(new{pattern=p,performed=f,pairComparisons=PairComparisons}));
 }
}
