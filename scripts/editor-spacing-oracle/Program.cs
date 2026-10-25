using System.Text.Json;
using StepManiaEditor;
var request=JsonDocument.Parse(File.ReadAllText(args[0])).RootElement;
double N(JsonElement e,string key)=>e.GetProperty(key).GetDouble();
var prefs=Preferences.Instance.PreferencesScroll;
prefs.SpacingMode=Enum.Parse<Editor.SpacingMode>(request.GetProperty("Mode").GetString());
Editor.Reverse=request.GetProperty("Reverse").GetBoolean();
var helper=EventSpacingHelper.GetSpacingHelper(new EditorChart());
var output=new List<object>();
foreach(var stage in request.GetProperty("Stages").EnumerateArray()) {
 var r=stage.GetProperty("Rate");
 var rate=new EditorRateAlteringEvent {Time=N(r,"Time"),Row=N(r,"Row"),Tempo=N(r,"Tempo"),SecondsPerRow=N(r,"SecondsPerRow"),RowsPerSecond=N(r,"RowsPerSecond"),Scroll=N(r,"Scroll")};
 var speeds=stage.GetProperty("Speeds");prefs.TimeBasedPixelsPerSecond=N(speeds,"Time");prefs.RowBasedPixelsPerRow=N(speeds,"Row");prefs.VariablePixelsPerSecondAtDefaultBPM=N(speeds,"Variable");
 helper.UpdatePpsAndPpr(rate,N(stage,"Interpolated"),N(stage,"Zoom"));
 var queries=new List<object>();
 foreach(var q in stage.GetProperty("Queries").EnumerateArray()) {
  var anchor=q.GetProperty("Anchor");var y=N(anchor,"Y");var time=N(anchor,"Time");var row=N(anchor,"Row");
  var e=new EditorEvent {Time=N(q,"Time"),Row=N(q,"Row")};
  var reg=q.GetProperty("Region");var region=new Region {Time=N(reg,"Time"),Row=N(reg,"Row"),TimeDuration=N(reg,"TimeDuration"),RowDuration=N(reg,"RowDuration"),Selection=reg.GetProperty("Selection").GetBoolean(),Y=N(reg,"DrawnY")};
  var location=helper.GetChartTimeAndRow(N(q,"Y"),y,time,row);
  var previous=helper.GetChartTimeAndRowFromPreviousRate(N(q,"Y"),y,time,row);
  queries.Add(new {Location=new[]{location.Item1,location.Item2},Previous=new[]{previous.Item1,previous.Item2},Y=helper.GetY(e,y,time,row),EventY=helper.GetY(e,y),RelativeY=helper.GetY(N(q,"Time"),N(q,"Row"),y),RowY=helper.GetYForRow(N(q,"Row"),y),RegionY=helper.GetRegionY(region,y),RegionHeight=helper.GetRegionH(region,y),Skip=helper.ShouldSkipDrawingRegion(region),Ends=helper.DoesRegionEndBeforeEvent(region,e)});
 }
 output.Add(new {Pps=helper.GetPps(),Ppr=helper.GetPpr(),Negative=helper.IsScrollRateNegative(),Queries=queries});
}
Console.WriteLine(JsonSerializer.Serialize(output));
