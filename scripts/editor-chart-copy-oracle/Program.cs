using System.Text.Json;
using StepManiaEditor;
var options = new JsonSerializerOptions { IncludeFields = true, WriteIndented = true };
var request = JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement;
var results = new List<object>();
foreach (var item in request.GetProperty("Cases").EnumerateArray()) {
    EditorChart ReadChart(JsonElement data) {
        var chart = new EditorChart(data.GetProperty("Layout").GetString(), data.GetProperty("Players").GetInt32());
        foreach (var value in data.GetProperty("Events").EnumerateArray()) chart.AddEvents([EditorEvent.Read(value, chart)]);
        return chart;
    }
    var source = ReadChart(item.GetProperty("Source"));
    var destinations = item.GetProperty("Destinations").EnumerateArray().Select(ReadChart).ToList();
    var types = item.GetProperty("Types").EnumerateArray().Select(type => type.GetString() switch {
        "Tempo" => typeof(EditorTempoEvent), "Signature" => typeof(EditorTimeSignatureEvent),
        "Delay" => typeof(EditorDelayEvent), "Label" => typeof(EditorLabelEvent),
        "Pattern" => typeof(EditorPatternEvent), "Attack" => typeof(EditorAttackEvent),
        _ => throw new ArgumentException("Unsupported oracle type")
    }).ToList();
    object[] Snapshot() => destinations.Select(chart => (object)chart.Project()).ToArray();
    var before = Snapshot(); var sourceBefore = source.Project();
    var action = new ActionCopyEventsBetweenCharts(source, types, destinations);
    action.Do(); var after = Snapshot(); action.Undo(); var undone = Snapshot(); action.Do(); var redone = Snapshot();
    results.Add(new { Name = item.GetProperty("Name").GetString(), Before = before, After = after, Undone = undone, Redone = redone, SourceBefore = sourceBefore, SourceAfter = source.Project() });
}
Console.WriteLine(JsonSerializer.Serialize(results, options));
