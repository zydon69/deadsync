using System.Text.Json;
using StepManiaEditor;
var request=JsonDocument.Parse(File.ReadAllText(args[0])).RootElement;
double N(JsonElement e,string key)=>e.GetProperty(key).GetDouble();
var prefs=Preferences.Instance.PreferencesScroll;
prefs.SpacingMode=Enum.Parse<Editor.SpacingMode>(request.GetProperty("Mode").GetString());
Editor.Reverse=request.GetProperty("Reverse").GetBoolean();
if(request.TryGetProperty("Operation",out var geometryOperation) && geometryOperation.GetString()=="Geometry") {
 var speeds=request.GetProperty("Speeds");prefs.TimeBasedPixelsPerSecond=N(speeds,"Time");prefs.RowBasedPixelsPerRow=N(speeds,"Row");prefs.VariablePixelsPerSecondAtDefaultBPM=N(speeds,"Variable");
 var rates=request.GetProperty("Rates").EnumerateArray().Select(r=>new EditorRateAlteringEvent{Time=N(r,"Time"),Row=N(r,"Row"),Tempo=N(r,"Tempo"),SecondsPerRow=N(r,"SecondsPerRow"),RowsPerSecond=N(r,"RowsPerSecond"),Scroll=N(r,"Scroll"),WarpRowsRemaining=N(r,"Warp"),StopTimeRemaining=N(r,"Stop"),Order=r.GetProperty("Order").GetInt32()}).ToArray();
 var index=request.GetProperty("Active").GetInt32();var focal=request.GetProperty("Focal");var helperGeometry=EventSpacingHelper.GetSpacingHelper(new EditorChart());
 var interpolation=N(request,"Interpolated");var zoom=N(request,"Zoom");
 void Update(int i)=>helperGeometry.UpdatePpsAndPpr(rates[i],interpolation,zoom);
 Update(index);var y=helperGeometry.GetY(rates[index],N(focal,"Y"),N(focal,"Time"),N(focal,"Row"));
 void Move(int target){
  while(index<target){var nextY=helperGeometry.GetY(rates[index+1],y,rates[index].Time,rates[index].Row);index++;Update(index);y=nextY;}
  while(index>target){var old=index;index--;Update(index);y=helperGeometry.GetY(rates[index],y,rates[old].Time,rates[old].Row);}
 }
 var outputGeometry=new List<object>();
 foreach(var step in request.GetProperty("Steps").EnumerateArray()) {
  var kind=step.GetProperty("Kind").GetString();double? eventY=null;
  if(kind=="Move")Move(step.GetProperty("Section").GetInt32());
  else if(kind=="HoldEnd"){
   var row=N(step,"Row");var selected=Enumerable.Range(0,rates.Length).Where(i=>rates[i].Row<row || rates[i].Row==row && rates[i].Order<11).DefaultIfEmpty(0).Last();Move(selected);
   eventY=helperGeometry.GetYForRow(row,y)+N(step,"CapHeight")*(Editor.Reverse ? -1.0 : 1.0);
  }
  else if(kind=="Event"){
   var row=N(step,"Row");var order=step.GetProperty("Order").GetInt32();var selected=Enumerable.Range(0,rates.Length).Where(i=>rates[i].Row<row || rates[i].Row==row && rates[i].Order<order).DefaultIfEmpty(0).Last();Move(selected);
   eventY=helperGeometry.GetY(N(step,"Time"),row,y,rates[index].Time,rates[index].Row);
  } else eventY=helperGeometry.GetY(N(step,"Time"),N(step,"Row"),y,rates[index].Time,rates[index].Row);
  outputGeometry.Add(new {Section=index,AnchorY=y,EventY=eventY});
 }
 Console.WriteLine(JsonSerializer.Serialize(outputGeometry));return;
}
if(request.TryGetProperty("Operation",out var operation) && operation.GetString()=="Search") {
 object Number(double value)=>double.IsFinite(value) ? value : (object)value.ToString("R",System.Globalization.CultureInfo.InvariantCulture);
 var speeds=request.GetProperty("Speeds");prefs.TimeBasedPixelsPerSecond=N(speeds,"Time");prefs.RowBasedPixelsPerRow=N(speeds,"Row");prefs.VariablePixelsPerSecondAtDefaultBPM=N(speeds,"Variable");
 var rates=request.GetProperty("Rates").EnumerateArray().Select(r=>new EditorRateAlteringEvent {Time=N(r,"Time"),Row=N(r,"Row"),Tempo=N(r,"Tempo"),SecondsPerRow=N(r,"SecondsPerRow"),RowsPerSecond=N(r,"RowsPerSecond"),Scroll=N(r,"Scroll"),WarpRowsRemaining=N(r,"Warp"),StopTimeRemaining=N(r,"Stop")}).ToArray();
 var values=new List<object>();
 foreach(var query in request.GetProperty("Queries").EnumerateArray()) {
  var focal=query.GetProperty("Focal");
  var search=new ActiveChartSearch {Chart=new EditorChart{Rates=new SearchRateList(rates,query.GetProperty("Active").GetInt32())},Position=new SearchPosition{ChartTime=N(focal,"Time"),ChartPosition=N(focal,"Row")},FocalPointScreenSpaceY=N(focal,"Y"),ZoomManager=new SearchZoom{Value=N(request,"Zoom")},Interpolated=N(request,"Interpolated")};
  var result=search.FindChartTimeAndRowForScreenSpaceY(query.GetProperty("Target").GetInt32());
  values.Add(new {Time=Number(result.Item1),Row=Number(result.Item2)});
 }
 Console.WriteLine(JsonSerializer.Serialize(values));return;
}
var helper=EventSpacingHelper.GetSpacingHelper(new EditorChart());
var output=new List<object>();
foreach(var stage in request.GetProperty("Stages").EnumerateArray()) {
 var r=stage.GetProperty("Rate");
 var rate=new EditorRateAlteringEvent {Time=N(r,"Time"),Row=N(r,"Row"),Tempo=N(r,"Tempo"),SecondsPerRow=N(r,"SecondsPerRow"),RowsPerSecond=N(r,"RowsPerSecond"),Scroll=N(r,"Scroll")};
 rate.WarpRowsRemaining=r.TryGetProperty("Warp",out var warp) ? warp.GetDouble() : 0;
 rate.StopTimeRemaining=r.TryGetProperty("Stop",out var stop) ? stop.GetDouble() : 0;
 var speeds=stage.GetProperty("Speeds");prefs.TimeBasedPixelsPerSecond=N(speeds,"Time");prefs.RowBasedPixelsPerRow=N(speeds,"Row");prefs.VariablePixelsPerSecondAtDefaultBPM=N(speeds,"Variable");
 helper.UpdatePpsAndPpr(rate,N(stage,"Interpolated"),N(stage,"Zoom"));
 var queries=new List<object>();
 foreach(var q in stage.GetProperty("Queries").EnumerateArray()) {
  var anchor=q.GetProperty("Anchor");var y=N(anchor,"Y");var time=N(anchor,"Time");var row=N(anchor,"Row");
  var e=new EditorEvent {Time=N(q,"Time"),Row=N(q,"Row")};
  var reg=q.GetProperty("Region");var region=new Region {Time=N(reg,"Time"),Row=N(reg,"Row"),TimeDuration=N(reg,"TimeDuration"),RowDuration=N(reg,"RowDuration"),Selection=reg.GetProperty("Selection").GetBoolean(),Y=N(reg,"DrawnY")};
  var location=helper.GetChartTimeAndRow(N(q,"Y"),y,time,row);
  var previous=helper.GetChartTimeAndRowFromPreviousRate(N(q,"Y"),y,time,row);
  queries.Add(new {Location=new[]{location.Item1,location.Item2},Previous=new[]{previous.Item1,previous.Item2},RowTime=rate.GetChartTimeFromPosition(N(q,"Row")),TimeRow=rate.GetChartPositionFromTime(N(q,"Time")),Y=helper.GetY(e,y,time,row),EventY=helper.GetY(e,y),RelativeY=helper.GetY(N(q,"Time"),N(q,"Row"),y),RowY=helper.GetYForRow(N(q,"Row"),y),RegionY=helper.GetRegionY(region,y),RegionHeight=helper.GetRegionH(region,y),Skip=helper.ShouldSkipDrawingRegion(region),Ends=helper.DoesRegionEndBeforeEvent(region,e)});
 }
 output.Add(new {Pps=helper.GetPps(),Ppr=helper.GetPpr(),Negative=helper.IsScrollRateNegative(),Queries=queries});
}
Console.WriteLine(JsonSerializer.Serialize(output));
