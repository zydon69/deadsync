using System.Text.Json;
using StepManiaEditor;
var request=JsonDocument.Parse(File.ReadAllText(args[0])).RootElement;
var tree=new RateAlteringEventTree();int index=0;
foreach(var rate in request.GetProperty("Rates").EnumerateArray())tree.Insert(new EditorRateAlteringEvent {Row=rate.GetProperty("Row").GetDouble(),Time=rate.GetProperty("Time").GetDouble(),Index=index++});
var output=new List<object>();
foreach(var query in request.GetProperty("Queries").EnumerateArray()) {
 Preferences.Instance.PreferencesScroll.SpacingMode=Enum.Parse<Editor.SpacingMode>(query.GetProperty("Mode").GetString());
 var result=tree.FindBest(new Position(query.GetProperty("Time").GetDouble(),query.GetProperty("Row").GetDouble()));
 int? selected=result!=null && result.MoveNext()?result.Current.Index:null;
 output.Add(new {Index=selected});
}
Console.WriteLine(JsonSerializer.Serialize(output));
