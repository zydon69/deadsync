using System.Text.Json;
using StepManiaEditor;

if (args.Length != 1) throw new ArgumentException("Usage: PatternClipboardOracle request.json");
var options = new JsonSerializerOptions { IncludeFields = true, WriteIndented = true };
var request = JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement;
var results = new List<object>();
foreach (var item in request.GetProperty("Cases").EnumerateArray()) {
    var chart = new EditorChart(item.GetProperty("Layout").GetString(), item.GetProperty("Players").GetInt32());
    foreach (var value in item.GetProperty("Destination").EnumerateArray()) chart.AddEvents([EditorEvent.Read(value, chart)]);
    var original = item.GetProperty("Copied").EnumerateArray().Select(value => EditorEvent.Read(value, chart)).ToList();
    // Original Editor.OnCopy clones immediately. Use the actual Pattern.Clone
    // override and Definition.Clone extracted from the pinned original here.
    var detached = original.Select(value => value.Clone()).ToList();
    var pairs = original.OfType<EditorPatternEvent>().Zip(detached.OfType<EditorPatternEvent>()).ToArray();
    var clonesMatchedBefore = pairs.All(pair => pair.First.MatchesDefinition(pair.Second));
    detached.Sort();
    var row = item.GetProperty("PasteRow").GetInt32();
    var offset = row - detached[0].GetRow();
    var action = new ActionPasteEvents(new Editor(item.GetProperty("ActivePlayer").GetInt32()), chart, detached, offset);
    foreach (var source in original.OfType<EditorPatternEvent>()) source.MutateSeed();
    var clonesIndependentAfter = pairs.All(pair => !pair.First.MatchesDefinition(pair.Second));
    var before = chart.Project();
    action.Do(); var after = chart.Project();
    action.Undo(); var undone = chart.Project();
    action.Do(); var redone = chart.Project();
    results.Add(new { Name = item.GetProperty("Name").GetString(), ClonesMatchedBefore = clonesMatchedBefore, ClonesIndependentAfter = clonesIndependentAfter, Before = before, After = after, Undone = undone, Redone = redone });
}
Console.WriteLine(JsonSerializer.Serialize(results, options));
