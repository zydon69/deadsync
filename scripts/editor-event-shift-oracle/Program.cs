using System.Text.Json;
using StepManiaEditor;
var options = new JsonSerializerOptions { IncludeFields = true, WriteIndented = true };
var request = JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement;
var results = new List<object>();
foreach (var item in request.GetProperty("Cases").EnumerateArray()) {
    var chart = new EditorChart(item.GetProperty("Layout").GetString(), item.GetProperty("Players").GetInt32());
    var selected = new List<EditorEvent>();
    foreach (var value in item.GetProperty("Events").EnumerateArray()) {
        var editorEvent = EditorEvent.Read(value, chart); chart.AddEvent(editorEvent);
        if (!value.TryGetProperty("Selected", out var choice) || choice.GetBoolean()) selected.Add(editorEvent);
    }
    // Original EditorChart rate initialization makes its first Tempo and first
    // TimeSignature immutable (EditorChart.cs1229–1329). Timing callbacks below
    // remain explicit adapters; no original clock dependency math is claimed.
    var tempo = chart.GetEvents().OfType<EditorTempoEvent>().FirstOrDefault(); if(tempo != null) tempo.IsPositionImmutable = true;
    var signature = chart.GetEvents().OfType<EditorTimeSignatureEvent>().FirstOrDefault(); if(signature != null) signature.IsPositionImmutable = true;
    var editor = new Editor(0);
    var before=chart.Project();
    var action=new ActionShiftSelectionRow(editor,chart,selected,item.GetProperty("Rows").GetInt32());
    action.Do(); var after=chart.Project(); var selectedAfter=editor.SelectedAfter;
    action.Undo(); var undone=chart.Project(); var selectedUndone=editor.SelectedAfter;
    action.Do(); var redone=chart.Project(); var selectedRedone=editor.SelectedAfter;
    results.Add(new {Name=item.GetProperty("Name").GetString(),Before=before,After=after,Undone=undone,Redone=redone,SelectedAfter=selectedAfter,SelectedUndone=selectedUndone,SelectedRedone=selectedRedone});
}
Console.WriteLine(JsonSerializer.Serialize(results,options));
