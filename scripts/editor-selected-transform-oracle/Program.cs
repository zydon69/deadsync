using Path = System.IO.Path;
using UndoableAction = StepManiaEditor.UndoableAction;
using System.Text.Json;
using Fumen;
using Fumen.Converters;
using Fumen.ChartDefinition;
using StepManiaEditor;
using StepManiaLibrary;
Logger.StartUp(new Logger.Config{QueueCapacity=0,CustomLogAction=_=>{}});
var request=JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement;var results=new List<object>();
var directory=Path.Combine(Path.GetTempPath(),"deadsync-selected-transform-"+Guid.NewGuid());Directory.CreateDirectory(directory);
try{foreach(var item in request.GetProperty("Cases").EnumerateArray()){
 var layout=item.GetProperty("Layout").GetString();var players=item.GetProperty("Players").GetInt32();
 var pad=await PadData.LoadPadData(layout,Path.Combine(args[1],"StepManiaLibrary",layout+".json"));
 var chart=new EditorChart(layout,players);var selected=new List<EditorEvent>();
 foreach(var value in item.GetProperty("Events").EnumerateArray()){var e=EditorEvent.Read(value,chart);chart.AddEvent(e);if(!value.TryGetProperty("Selected",out var flag)||flag.GetBoolean())selected.Add(e);}
 var editor=new Editor(0);editor.Pad=pad;var before=chart.Project();
 UndoableAction action=item.GetProperty("Operation").GetString() switch {
  "Mirror"=>new ActionMirrorSelection(editor,chart,selected),"Flip"=>new ActionFlipSelection(editor,chart,selected),"MirrorAndFlip"=>new ActionMirrorAndFlipSelection(editor,chart,selected),
  "Left"=>new ActionShiftSelectionLane(editor,chart,selected,false,false),"Right"=>new ActionShiftSelectionLane(editor,chart,selected,true,false),"LeftWrap"=>new ActionShiftSelectionLane(editor,chart,selected,false,true),"RightWrap"=>new ActionShiftSelectionLane(editor,chart,selected,true,true),
  "Rows"=>new ActionShiftSelectionRow(editor,chart,selected,item.GetProperty("Rows").GetInt32()),
  "Assign"=>new ActionChangeNotePlayer(editor,chart,selected,item.GetProperty("PlayerA").GetInt32()),"SwapPlayers"=>new ActionSwapNotePlayer(editor,chart,selected,item.GetProperty("PlayerA").GetInt32(),item.GetProperty("PlayerB").GetInt32()),_=>throw new Exception("operation")};
 action.Do();var after=chart.Project();var selectedAfter=editor.SelectedAfter;action.Undo();var undone=chart.Project();action.Do();var redone=chart.Project();
 var serialized=new List<object>();foreach(var format in new[]{"sm","ssc"}){
  var path=Path.Combine(directory,Guid.NewGuid()+"."+format);var song=chart.Song(layout,players);var config=new SMWriterBase.SMWriterBaseConfig{Song=song,FilePath=path,FallbackChart=song.Charts[0]};var saved=format=="sm"?new SMWriter(config).Save():new SSCWriter(config).Save();
  var read=await Reader.CreateReader(path).LoadAsync(CancellationToken.None);serialized.Add(new{Format=format,Saved=saved,Bytes=Convert.ToBase64String(File.ReadAllBytes(path)),Charts=read.Charts.Select(c=>new{Players=c.NumPlayers,Notes=c.Layers.SelectMany(l=>l.Events).OfType<LaneNote>().Select(n=>new{Row=n.IntegerPosition,Lane=n.Lane,Player=n.Player,Source=n.SourceType}).ToArray()}).ToArray()});
 }
 results.Add(new{Name=item.GetProperty("Name").GetString(),Before=before,After=after,Undone=undone,Redone=redone,SelectedAfter=selectedAfter,Serialized=serialized});
}var snapReference=new ShiftRowsReference();var shifts=new[]{48,24,16,12,8,6,4,3,1,0}.Select(rows=>{snapReference.SnapManager.Rows=rows;return new{SnapRows=rows,ShiftRows=snapReference.GetShiftNotesRows()};}).ToArray();Console.Write(JsonSerializer.Serialize(new{Cases=results,ShiftSteps=shifts}));}finally{Directory.Delete(directory,true);}
