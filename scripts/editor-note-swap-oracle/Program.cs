using System.Text.Json;
using Path = System.IO.Path;
using ActionQueue = StepManiaEditor.ActionQueue;
using Fumen;
using Fumen.Converters;
using Fumen.ChartDefinition;
using StepManiaEditor;
using static Fumen.Converters.SMWriterBase;
Logger.StartUp(new Logger.Config{QueueCapacity=0,CustomLogAction=_=>{}});
var request=JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement;var results=new List<object>();var directory=Path.Combine(Path.GetTempPath(),"deadsync-note-swap-"+Guid.NewGuid());Directory.CreateDirectory(directory);
try{foreach(var item in request.GetProperty("Cases").EnumerateArray()){
 var chart=new EditorChart();var selected=new List<EditorEvent>();foreach(var value in item.GetProperty("Events").EnumerateArray()){var e=EditorEvent.Read(value,chart);chart.Values.Add(e);if(!value.TryGetProperty("Selected",out var flag)||flag.GetBoolean())selected.Add(e);}var editor=new Editor();var menu=new Menu(editor);var before=chart.Project();menu.Invoke(item.GetProperty("Method").GetString(),chart,selected);var action=ActionQueue.Instance.Last;var after=chart.Project();var selectedAfter=editor.Selected;action.Undo();var undone=chart.Project();action.Do();var redone=chart.Project();var serialized=new List<object>();foreach(var format in new[]{"sm","ssc"}){var path=Path.Combine(directory,Guid.NewGuid()+"."+format);var song=chart.Song();var config=new SMWriterBaseConfig{Song=song,FilePath=path,FallbackChart=song.Charts[0]};var saved=format=="sm"?new SMWriter(config).Save():new SSCWriter(config).Save();var read=await Reader.CreateReader(path).LoadAsync(CancellationToken.None);serialized.Add(new{Format=format,Saved=saved,Bytes=Convert.ToBase64String(File.ReadAllBytes(path)),Charts=read.Charts.Select(c=>new{Players=c.NumPlayers,Notes=c.Layers.SelectMany(l=>l.Events).OfType<LaneNote>().Select(n=>new{Row=n.IntegerPosition,Lane=n.Lane,Player=n.Player,Source=n.SourceType}).ToArray()}).ToArray()});}results.Add(new{Name=item.GetProperty("Name").GetString(),Before=before,After=after,Undone=undone,Redone=redone,SelectedAfter=selectedAfter,Serialized=serialized});}Console.Write(JsonSerializer.Serialize(new{Cases=results}));}finally{Directory.Delete(directory,true);}
