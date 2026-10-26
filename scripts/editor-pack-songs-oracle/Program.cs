using System.Globalization;
using System.Text.Json;
using System.Text.Json.Serialization;
using StepManiaEditor;
using Fumen;
using Fumen.Converters;
using Path=System.IO.Path;
var request=JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement;
var root=Path.Combine(Path.GetTempPath(),"deadsync-pack-songs-oracle-"+Guid.NewGuid());Directory.CreateDirectory(root);
Logger.StartUp(new Logger.Config {QueueCapacity=0,CustomLogAction=_=>{}});
var rows=new List<object>();var loaded=new List<PackSong>();var orders=new Dictionary<string,string[]>();
try {
 foreach(var item in request.GetProperty("Songs").EnumerateArray()) {
  var name=item.GetProperty("Name").GetString();var dir=new DirectoryInfo(Path.Combine(root,name));dir.Create();
  foreach(var file in item.GetProperty("Files").EnumerateArray()) await File.WriteAllBytesAsync(Path.Combine(dir.FullName,file.GetProperty("Name").GetString()),Convert.FromBase64String(file.GetProperty("Bytes").GetString()));
  FileInfo sm=null,ssc=null;var enumeration=dir.GetFiles();foreach(var file in enumeration){if(file.Extension==".ssc")ssc=file;else if(file.Extension==".sm")sm=file;}
  if(sm==null&&ssc==null){rows.Add(new{Name=name,Loaded=false,Selected=(string)null,Excluded=true});continue;}
  var song=new PackSong(dir,ssc,sm);bool success=false;string error=null;
  try{success=await song.LoadAsync(CancellationToken.None);}catch(Exception e){error=e.GetType().Name;}
  var data=song.GetSong();string credit=null;if(data!=null)data.Extras.TryGetExtra(SMCommon.TagCredit,out credit,true);
  int[] sortedIndices=null;int[][] comparisons=null;string sortError=null;
  if(data!=null){var original=data.Charts.ToArray();comparisons=original.Select(first=>original.Select(second=>PackChartComparer.Compare(first,second)).ToArray()).ToArray();var sorted=original.ToList();try{sorted.Sort(new PackChartComparer());sortedIndices=sorted.Select(chart=>Array.FindIndex(original,candidate=>Object.ReferenceEquals(candidate,chart))).ToArray();}catch(Exception exception){sortError=exception.GetType().Name;}}
  var cancelled=false;using(var cancellation=new CancellationTokenSource()){cancellation.Cancel();try{await song.LoadAsync(cancellation.Token);}catch(OperationCanceledException){cancelled=true;}}
  rows.Add(new{Name=name,Loaded=success,Selected=song.GetFileInfo().Name,Excluded=false,Error=error,Title=data?.Title,TitleTranslit=data?.TitleTransliteration,Artist=data?.Artist,Credit=credit,Ratings=song.GetRatingsString(),Charts=data?.Charts.Select(c=>new{Type=c.Type,Rating=c.DifficultyRating,Difficulty=c.DifficultyType}).ToArray(),Cancelled=cancelled,SortedIndices=sortedIndices,Comparisons=comparisons,SortError=sortError,Enumeration=enumeration.Select(f=>f.Name).ToArray()});
  if(success)loaded.Add(song);
 }
 foreach(var culture in request.GetProperty("Cultures").EnumerateArray()){CultureInfo.CurrentCulture=new CultureInfo(culture.GetString());var sorted=new List<PackSong>(loaded);sorted.Sort(new PackSongComparer());orders[culture.GetString()]=sorted.Select(s=>s.GetDirectoryName()).ToArray();}
 Console.WriteLine(JsonSerializer.Serialize(new{Songs=rows,Orders=orders},new JsonSerializerOptions{WriteIndented=true,NumberHandling=JsonNumberHandling.AllowNamedFloatingPointLiterals}));
}finally{Directory.Delete(root,true);}
