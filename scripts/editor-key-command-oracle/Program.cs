using Microsoft.Xna.Framework.Input;
using StepManiaEditor;
var output=new List<string>();
var p=Preferences.Instance.PreferencesKeyBinds;
p.Bindings["save"]=new(){new[]{Keys.LeftControl,Keys.S}};
var manager=new KeyCommandManager();
manager.Register(new("save","save",()=>output.Add("press"),false,()=>output.Add("release")));
Keyboard.Down=new[]{Keys.S};manager.Update(0);
Keyboard.Down=new[]{Keys.S,Keys.LeftControl};manager.Update(.1);
Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(new{scenario="final-before-modifier",effects=output}));
var equalEffects=new List<string>();
p.Bindings["equalSave"]=new(){new[]{Keys.Enter}};
p.Bindings["equalPlay"]=new(){new[]{Keys.Enter}};
var equalManager=new KeyCommandManager();
equalManager.Register(new("save","equalSave",()=>equalEffects.Add("save"),true,()=>equalEffects.Add("save:release")));
equalManager.Register(new("play","equalPlay",()=>equalEffects.Add("play"),true,()=>equalEffects.Add("play:release")));
var conflicts=equalManager.GetConflictingCommands("equalSave",new[]{Keys.Enter});
var frames=new List<object>();
void Frame(double time,params Keys[] down) {
    Keyboard.Down=down;equalEffects.Clear();equalManager.Update(time);
    frames.Add(new{time,down=down.Select(key=>key.ToString()).ToArray(),effects=equalEffects.ToArray()});
}
Frame(0);Frame(.1,Keys.Enter);Frame(.6,Keys.Enter);Frame(.601,Keys.Enter);
Frame(.62,Keys.Enter);Frame(.621,Keys.Enter);Frame(.7);
Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(new{scenario="equal-chords",conflicts,frames}));
namespace Microsoft.Xna.Framework.Input {
 public enum Keys {LeftControl,RightControl,LeftShift,RightShift,LeftAlt,RightAlt,LeftWindows,RightWindows,S,Enter}
 public struct KeyboardState {readonly HashSet<Keys> down; public KeyboardState(Keys[] keys){down=new(keys);} public bool IsKeyDown(Keys key)=>down!=null&&down.Contains(key);}
 public static class Keyboard {public static Keys[] Down=[];public static KeyboardState GetState()=>new(Down);}
}
namespace Fumen {public interface IObserver<T>{void OnNotify(string eventId,T notifier,object payload);}}
namespace StepManiaEditor {
 public class Preferences {public static Preferences Instance=new();public PreferencesKeyBinds PreferencesKeyBinds=new();}
 public class PreferencesKeyBinds {public const string NotificationKeyBindingChanged="changed"; public Dictionary<string,List<Keys[]>> Bindings=new(); public void AddObserver(Fumen.IObserver<PreferencesKeyBinds> observer){} public bool BlocksInput(string id)=>true;}
 public static class Utils {public static T GetValueFromFieldOrProperty<T>(PreferencesKeyBinds p,string id)=>(T)(object)p.Bindings[id];}
}
