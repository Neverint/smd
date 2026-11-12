//! Hardware Health: battery report parsing and cleaning up sensor readings.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatteryInfo {
    pub name: String,
    /// mWh
    pub design_capacity: u64,
    /// mWh
    pub full_charge_capacity: u64,
    pub cycle_count: Option<u32>,
}

impl BatteryInfo {
    /// Full charge vs design capacity, in percent.
    pub fn health_percent(&self) -> f64 {
        if self.design_capacity == 0 {
            0.0
        } else {
            self.full_charge_capacity as f64 * 100.0 / self.design_capacity as f64
        }
    }
}

fn tag<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim())
}

/// Reads the `<Battery>` entries of `powercfg /batteryreport /xml`.
pub fn parse_battery_report(xml: &str) -> Vec<BatteryInfo> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<Battery>") {
        let after = &rest[i + "<Battery>".len()..];
        let Some(end) = after.find("</Battery>") else {
            break;
        };
        let block = &after[..end];
        let num = |n: &str| tag(block, n).and_then(|v| v.parse::<u64>().ok());
        if let (Some(design), Some(full)) = (num("DesignCapacity"), num("FullChargeCapacity"))
            && design > 0
        {
            let name = [tag(block, "Manufacturer"), tag(block, "Id")]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            out.push(BatteryInfo {
                name,
                design_capacity: design,
                full_charge_capacity: full,
                cycle_count: num("CycleCount")
                    .and_then(|c| u32::try_from(c).ok())
                    .filter(|c| *c > 0),
            });
        }
        rest = &after[end..];
    }
    out
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorReading {
    pub name: String,
    /// `Temperature`, `Fan`, `Load`, `Clock`, `Power`, `Control`
    pub kind: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareReading {
    pub name: String,
    /// LibreHardwareMonitor's hardware type: `Cpu`, `GpuAmd`, `GpuNvidia`, `GpuIntel`, `Memory`, `Storage`, `Motherboard`, ...
    pub kind: String,
    pub sensors: Vec<SensorReading>,
}

/// Drops readings that only mean "not available": a temperature of 0 °C or less,
/// out-of-range temperatures, and CPU power without admin rights (reported as 0).
/// A fan at 0 RPM is kept: GPUs stop their fans when idle.
pub fn clean_readings(hardware: Vec<HardwareReading>, elevated: bool) -> Vec<HardwareReading> {
    hardware
        .into_iter()
        .map(|mut h| {
            let cpu = h.kind.eq_ignore_ascii_case("Cpu");
            h.sensors.retain(|s| match s.kind.as_str() {
                "Temperature" => s.value > 0.0 && s.value < 150.0,
                "Power" => !(cpu && !elevated && s.value <= 0.0) && s.value >= 0.0,
                "Fan" => s.value >= 0.0 && s.value < 20_000.0,
                "Load" | "Control" => (0.0..=100.0).contains(&s.value),
                _ => s.value.is_finite(),
            });
            h
        })
        .filter(|h| !h.sensors.is_empty())
        .collect()
}

/// The hottest reading of a hardware item (for the summary cards).
pub fn max_temperature(h: &HardwareReading) -> Option<f64> {
    h.sensors
        .iter()
        .filter(|s| s.kind == "Temperature")
        .map(|s| s.value)
        .reduce(f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = r#"<?xml version="1.0"?>
<BatteryReport><Batteries>
  <Battery><Id>DELL 7FHHV</Id><Manufacturer>SMP</Manufacturer><DesignCapacity>56000</DesignCapacity><FullChargeCapacity>44800</FullChargeCapacity><CycleCount>312</CycleCount></Battery>
  <Battery><Id>X</Id><DesignCapacity>0</DesignCapacity><FullChargeCapacity>0</FullChargeCapacity></Battery>
</Batteries></BatteryReport>"#;

    #[test]
    fn parses_battery_report() {
        let b = parse_battery_report(REPORT);
        assert_eq!(
            b.len(),
            1,
            "batteries without a design capacity are skipped"
        );
        assert_eq!(b[0].name, "SMP DELL 7FHHV");
        assert_eq!(b[0].cycle_count, Some(312));
        assert!((b[0].health_percent() - 80.0).abs() < 0.01);
        assert!(parse_battery_report("<BatteryReport/>").is_empty());
    }

    fn s(name: &str, kind: &str, value: f64) -> SensorReading {
        SensorReading {
            name: name.into(),
            kind: kind.into(),
            value,
        }
    }

    #[test]
    fn hides_unavailable_readings() {
        let input = vec![
            HardwareReading {
                name: "Ryzen".into(),
                kind: "Cpu".into(),
                sensors: vec![
                    s("Tctl", "Temperature", 0.0),
                    s("Package", "Power", 0.0),
                    s("Core #1", "Load", 12.0),
                ],
            },
            HardwareReading {
                name: "RX 6700 XT".into(),
                kind: "GpuAmd".into(),
                sensors: vec![
                    s("Core", "Temperature", 32.0),
                    s("Fan", "Fan", 0.0),
                    s("Package", "Power", 7.0),
                ],
            },
            HardwareReading {
                name: "Board".into(),
                kind: "Motherboard".into(),
                sensors: vec![s("T", "Temperature", -1.0)],
            },
        ];
        let out = clean_readings(input.clone(), false);
        assert_eq!(out.len(), 2, "hardware with nothing left is dropped");
        assert_eq!(out[0].sensors, vec![s("Core #1", "Load", 12.0)]);
        assert_eq!(out[1].sensors.len(), 3, "an idle GPU fan at 0 RPM is real");
        assert_eq!(max_temperature(&out[1]), Some(32.0));
        let elevated = clean_readings(input, true);
        assert_eq!(
            elevated[0].sensors.len(),
            2,
            "CPU power of 0 W is kept when elevated"
        );
    }
}
