// smd-hwmon: reads hardware sensors with LibreHardwareMonitorLib and prints JSON lines.
//
// Output, one line every 2 s:
//   {"elevated":false,"hardware":[{"name":"...","kind":"Cpu","sensors":[{"name":"...","kind":"Temperature","value":45.0}]}]}
// The process exits when stdin reaches end-of-file, so it never outlives the app.
// Without admin rights no kernel driver is loaded: CPU temperatures and motherboard fans
// are then unavailable, GPU and some other readings still work.

using System.Security.Principal;
using System.Text.Json;
using System.Text.Json.Serialization;
using LibreHardwareMonitor.Hardware;

var computer = new Computer
{
    IsCpuEnabled = true,
    IsGpuEnabled = true,
    IsMemoryEnabled = true,
    IsMotherboardEnabled = true,
    IsControllerEnabled = true,
    IsStorageEnabled = true,
};

var elevated = new WindowsPrincipal(WindowsIdentity.GetCurrent()).IsInRole(WindowsBuiltInRole.Administrator);
var wanted = new HashSet<SensorType>
{
    SensorType.Temperature, SensorType.Fan, SensorType.Load, SensorType.Clock, SensorType.Power, SensorType.Control,
};
var json = new JsonSerializerOptions { PropertyNamingPolicy = JsonNamingPolicy.CamelCase, DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull };

try
{
    computer.Open();
}
catch (Exception e)
{
    Console.Out.WriteLine(JsonSerializer.Serialize(new Snapshot(elevated, [], e.Message), json));
    Console.Out.Flush();
}

// Stop when the app closes our stdin.
var stop = new CancellationTokenSource();
_ = Task.Run(() =>
{
    try
    {
        while (Console.In.Read() != -1) { }
    }
    catch
    {
        // Pipe broken: the app is gone.
    }
    stop.Cancel();
});

while (!stop.IsCancellationRequested)
{
    var hardware = new List<HardwareReading>();
    foreach (var hw in computer.Hardware)
    {
        Collect(hw, hardware);
    }
    try
    {
        Console.Out.WriteLine(JsonSerializer.Serialize(new Snapshot(elevated, hardware, null), json));
        Console.Out.Flush();
    }
    catch (IOException)
    {
        break;
    }
    try
    {
        await Task.Delay(TimeSpan.FromSeconds(2), stop.Token);
    }
    catch (TaskCanceledException)
    {
        break;
    }
}

computer.Close();
return 0;

void Collect(IHardware hw, List<HardwareReading> into)
{
    try
    {
        hw.Update();
    }
    catch
    {
        return;
    }
    var sensors = hw.Sensors
        .Where(s => wanted.Contains(s.SensorType) && s.Value is { } v && float.IsFinite(v))
        .Select(s => new SensorReading(s.Name, s.SensorType.ToString(), Math.Round((double)s.Value!.Value, 1)))
        .ToList();
    if (sensors.Count > 0)
    {
        into.Add(new HardwareReading(hw.Name, hw.HardwareType.ToString(), sensors));
    }
    foreach (var sub in hw.SubHardware)
    {
        Collect(sub, into);
    }
}

record SensorReading(string Name, string Kind, double Value);
record HardwareReading(string Name, string Kind, List<SensorReading> Sensors);
record Snapshot(bool Elevated, List<HardwareReading> Hardware, string? Error);
