using Fumen;
using Fumen.ChartDefinition;
using Fumen.Converters;
using System.Text.Json;
var results = new List<object>();
foreach (var kind in new[]{"Tempo", "Signature"}) foreach (var equal in new[]{false,true}) {
    Event first = kind == "Tempo" ? new Tempo(180) : new TimeSignature(new Fraction(2,2),0);
    Event second = kind == "Tempo" ? new Tempo(equal ? 180 : 240) : new TimeSignature(new Fraction(equal ? 2 : 3,equal ? 2 : 4),0);
    first.IntegerPosition=48; second.IntegerPosition=48;
    var tree = new RedBlackTree<Node>(); tree.Insert(new Node(first));
    bool rejected=false; string error=null;
    try {tree.Insert(new Node(second));} catch(ArgumentException e){rejected=true;error=e.GetType().Name;}
    results.Add(new {kind,equal,comparison=new SMCommon.SMEventComparer().Compare(first,second),rejected,error});
}
Console.WriteLine(JsonSerializer.Serialize(results,new JsonSerializerOptions{WriteIndented=true}));
sealed class Node(Event value) : IComparable<Node> {
    public Event Value=value;
    public int CompareTo(Node other)=>new SMCommon.SMEventComparer().Compare(Value,other.Value);
}
