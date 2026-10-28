using System.Text.Json;
using Fumen;
using Fumen.ChartDefinition;
using StepManiaEditor;
using UndoableAction = StepManiaEditor.UndoableAction;
Logger.StartUp(new Logger.Config{QueueCapacity=0,CustomLogAction=_=>{}});
var request=JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement;var result=new List<object>();
foreach(var item in request.GetProperty("Cases").EnumerateArray()){
 var chart=new EditorChart();var chosen=new List<EditorEvent>();foreach(var value in item.GetProperty("Events").EnumerateArray()){var row=value.GetProperty("Row").GetInt32();var kind=value.GetProperty("Kind").GetString();EditorEvent e=kind switch{"Tempo"=>new EditorTempoEvent(chart,new Tempo(value.GetProperty("Value").GetDouble()){IntegerPosition=row}),"Signature"=>new EditorTimeSignatureEvent(chart,new TimeSignature(new Fraction(4,4),0){IntegerPosition=row}),"Warp"=>new EditorWarpEvent(chart,new Warp(value.GetProperty("Value").GetInt32()){IntegerPosition=row}),"Stop"=>new EditorStopEvent(chart,new Stop(value.GetProperty("Value").GetDouble()){IntegerPosition=row}),"Delay"=>new EditorDelayEvent(chart,new Stop(value.GetProperty("Value").GetDouble(),true){IntegerPosition=row}),_=>throw new Exception("kind")};chart.EditorEvents.Add(e);if(!value.TryGetProperty("Selected",out var selected)||selected.GetBoolean())chosen.Add(e);}chart.Refresh();
 var editor=new Editor();var before=Project(chart);var all=item.GetProperty("All").GetBoolean();UndoableAction action=item.GetProperty("Direction").GetString()=="WarpToStop"?(all?new ActionChangeWarpsToNegativeStops(editor,chart):new ActionChangeWarpsToNegativeStops(editor,chart,chosen)):(all?new ActionChangeNegativeStopsToWarps(editor,chart):new ActionChangeNegativeStopsToWarps(editor,chart,chosen));action.Run();var after=Project(chart);var selectedAfter=editor.Selected;action.Undo();var undone=Project(chart);action.Run();var redone=Project(chart);var selectedRedone=editor.Selected;result.Add(new{Name=item.GetProperty("Name").GetString(),Before=before,After=after,Undone=undone,Redone=redone,SelectedAfter=selectedAfter,SelectedRedone=selectedRedone});}
Console.Write(JsonSerializer.Serialize(new{Cases=result}));
static object[] Project(EditorChart chart)=>chart.EditorEvents.Select(e=>Editor.Project(e)).ToArray();
