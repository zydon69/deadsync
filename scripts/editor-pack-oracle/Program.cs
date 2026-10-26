using System.Globalization;
using Path = System.IO.Path;
using System.Text.Json;
using Fumen;
using Fumen.Converters;
using PackOracle;
CultureInfo.CurrentCulture = CultureInfo.InvariantCulture;
var options = new JsonSerializerOptions { WriteIndented = true };
var request = JsonDocument.Parse(File.ReadAllBytes(args[0])).RootElement;
var root = Path.Combine(Path.GetTempPath(), "deadsync-pack-oracle-" + Guid.NewGuid());
Directory.CreateDirectory(root);
var messages = new List<Logger.LogMessage>();
Logger.StartUp(new Logger.Config { QueueCapacity=0, CustomLogAction=message=>messages.Add(message) });
object Project(ItgManiaPack pack) => pack == null ? null : new {
    pack.Title, pack.TitleTransliteration, pack.TitleSort, pack.Series, pack.Year, pack.Banner,
    SyncOffset=(int)pack.SyncOffset, SyncName=pack.SyncOffset.ToString()
};
var loadedResults = new List<object>(); var creationResults = new List<object>();
try {
    foreach (var item in request.GetProperty("Loads").EnumerateArray()) {
        messages.Clear();var name=item.GetProperty("Name").GetString();
        var path=Path.Combine(root,name+".ini");
        await File.WriteAllBytesAsync(path,Convert.FromBase64String(item.GetProperty("Bytes").GetString()));
        var pack=await ItgManiaPack.LoadAsync(path,CancellationToken.None);
        var warnings=messages.Count(message=>message.Level==LogLevel.Warn);var errors=messages.Count(message=>message.Level==LogLevel.Error);
        var data=Project(pack); bool? cloneMatches=null,cloneIndependent=null,dirtyBefore=null,dirtyEdited=null,dirtyRestored=null,dirtySaved=null;
        string written=null; object reloaded=null;
        if(pack!=null) {
            var clone=(ItgManiaPack)pack.Clone();cloneMatches=pack.Matches(clone);var originalTitle=pack.Title;
            clone.Title=(clone.Title??"")+" clone";cloneIndependent=!pack.Matches(clone)&&pack.Title==originalTitle;
            var writtenPath=Path.Combine(root,name+"-written.ini");
            if(!pack.Save(writtenPath)) throw new Exception("Original pack save failed");
            written=Convert.ToBase64String(await File.ReadAllBytesAsync(writtenPath));reloaded=Project(await ItgManiaPack.LoadAsync(writtenPath,CancellationToken.None));
            var editor=EditorItgManiaPack.FromLoaded(Path.Combine(root,name+"-editor.ini"),pack);
            dirtyBefore=editor.HasUnsavedChanges();editor.Pack.Title=(originalTitle??"")+" edited";dirtyEdited=editor.HasUnsavedChanges();editor.Pack.Title=originalTitle;dirtyRestored=editor.HasUnsavedChanges();
            editor.Pack.Title=(originalTitle??"")+" saved";await editor.SaveAsync();dirtySaved=editor.HasUnsavedChanges();
        }
        loadedResults.Add(new {Name=name,Loaded=pack!=null,Data=data,Warnings=warnings,Errors=errors,CloneMatches=cloneMatches,CloneIndependent=cloneIndependent,Written=written,Reloaded=reloaded,DirtyBefore=dirtyBefore,DirtyEdited=dirtyEdited,DirtyRestored=dirtyRestored,DirtySaved=dirtySaved});
    }
    foreach(var item in request.GetProperty("Creations").EnumerateArray()) {
        var active=item.GetProperty("Active");double? activeSync=active.ValueKind==JsonValueKind.Null?null:active.GetDouble();
        OracleClock.Year=item.GetProperty("Year").GetInt32();var sync=Defaults.ChooseSync(activeSync,item.GetProperty("Preferred").GetDouble());
        var name=item.GetProperty("Name").GetString();var path=Path.Combine(root,name+"-new.ini");
        var editor=await EditorItgManiaPack.CreateNewPack(new EditorPack(),path,item.GetProperty("Title").GetString(),item.GetProperty("Banner").GetString(),sync);
        creationResults.Add(new {Name=name,Data=Project(editor.Pack),Dirty=editor.HasUnsavedChanges(),Written=Convert.ToBase64String(await File.ReadAllBytesAsync(path))});
    }
} finally {Logger.Shutdown();Directory.Delete(root,true);}
Console.WriteLine(JsonSerializer.Serialize(new {Loads=loadedResults,Creations=creationResults},options));
