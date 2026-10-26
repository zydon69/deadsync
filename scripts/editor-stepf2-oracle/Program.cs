// Development-only differential oracle. All note parsing and writing below is
// performed by the actual pinned Fumen reader/writer, not a reimplementation.
using System.Text.Json;
using Fumen.ChartDefinition;
using Fumen.Converters;
using static Fumen.Converters.SMCommon;

if (args.Length != 1) throw new ArgumentException("Usage: StepF2Oracle request.json");
System.Globalization.CultureInfo.CurrentCulture = System.Globalization.CultureInfo.InvariantCulture;
var request = JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement;
var results = new List<object>();
foreach (var item in request.GetProperty("Cases").EnumerateArray()) {
    var format = item.GetProperty("Format").GetString()!;
    var path = Path.Combine(Path.GetTempPath(), Guid.NewGuid() + "." + format);
    var written = Path.Combine(Path.GetTempPath(), Guid.NewGuid() + "." + format);
    try {
        await File.WriteAllTextAsync(path, item.GetProperty("Content").GetString()!);
        Reader reader = format == "sm" ? new SMReader(path) : new SSCReader(path);
        var song = await reader.LoadAsync(CancellationToken.None);
        var read = Project(song);
        var config = new SMWriterBase.SMWriterBaseConfig {
            Song = song, FilePath = written, FallbackChart = song.Charts.FirstOrDefault(),
            UseStepF2ForPumpMultiplayerCharts = true,
        };
        bool saved = format == "sm" ? new SMWriter(config).Save() : new SSCWriter(config).Save();
        Reader rereader = format == "sm" ? new SMReader(written) : new SSCReader(written);
        var reloaded = saved ? Project(await rereader.LoadAsync(CancellationToken.None)) : null;
        results.Add(new { Name = item.GetProperty("Name").GetString(), Read = read, Saved = saved, Reloaded = reloaded });
    } finally { File.Delete(path); File.Delete(written); }
}
Console.WriteLine(JsonSerializer.Serialize(results, new JsonSerializerOptions { WriteIndented = true }));

static object[] Project(Song song) => song.Charts.Select(chart => {
    var lanes = chart.Layers[0].Events.OfType<LaneNote>().ToArray();
    var notes = lanes.Where(note => note is not LaneHoldEndNote).OrderBy(note => note.IntegerPosition).ThenBy(note => note.Lane).Select(note => {
        var end = note is LaneHoldStartNote ? (int?)lanes.OfType<LaneHoldEndNote>().First(tail => tail.Player == note.Player && tail.Lane == note.Lane && tail.IntegerPosition > note.IntegerPosition).IntegerPosition : null;
        var kind = note is LaneHoldStartNote ? (note.SourceType == "4" ? "roll" : "hold") : note.SourceType switch { "F" => "fake", "M" => "mine", "L" => "lift", "K" => "keysound", _ => "tap" };
        bool Flag(string tag) { note.Extras.TryGetExtra(tag, out bool flag, true); return flag; }
        return new { Row = note.IntegerPosition, Lane = note.Lane, Player = note.Player, Kind = kind, End = end,
            FakeFlag = Flag(TagFumenStepF2Fake), Sudden = Flag(TagFumenStepF2Sudden), Vanish = Flag(TagFumenStepF2Vanish), Hidden = Flag(TagFumenStepF2Hidden) };
    }).ToArray();
    return (object)new { Layout = chart.Type, Players = chart.NumPlayers, Notes = notes };
}).ToArray();
