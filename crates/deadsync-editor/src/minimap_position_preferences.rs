//! Original MiniMap mount/select preferences and per-mount signed offsets.
use crate::Error;
use serde_json::{Map, Value};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mount {
    RightSideOfWindow,
    LeftSideOfWindow,
    FocusedChartWithoutScaling,
    FocusedChartWithScaling,
}
impl Mount {
    pub const ALL: [Self; 4] = [
        Self::RightSideOfWindow,
        Self::LeftSideOfWindow,
        Self::FocusedChartWithoutScaling,
        Self::FocusedChartWithScaling,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::RightSideOfWindow => "RightSideOfWindow",
            Self::LeftSideOfWindow => "LeftSideOfWindow",
            Self::FocusedChartWithoutScaling => "FocusedChartWithoutScaling",
            Self::FocusedChartWithScaling => "FocusedChartWithScaling",
        }
    }
    fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mount| mount.name() == value)
    }
    fn index(self) -> usize {
        match self {
            Self::RightSideOfWindow => 0,
            Self::LeftSideOfWindow => 1,
            Self::FocusedChartWithoutScaling => 2,
            Self::FocusedChartWithScaling => 3,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectMode {
    MoveToCursor,
    MoveToSelectedArea,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub mount: Mount,
    pub select_mode: SelectMode,
    offsets: [i32; 4],
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            mount: Mount::LeftSideOfWindow,
            select_mode: SelectMode::MoveToCursor,
            offsets: [110, 110, 10, 10],
        }
    }
}
impl Preferences {
    pub fn offset(self, mount: Mount) -> i32 {
        self.offsets[mount.index()]
    }
    pub fn set_offset(&mut self, mount: Mount, value: i32) {
        self.offsets[mount.index()] = value;
    }
    pub fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let mut result = Self::default();
        // Original permissive enum converter restores the registered default.
        result.mount = fields
            .get("MiniMapPosition")
            .and_then(Value::as_str)
            .and_then(Mount::parse)
            .unwrap_or(Mount::LeftSideOfWindow);
        result.select_mode = match fields.get("MiniMapSelectMode").and_then(Value::as_str) {
            Some("MoveToSelectedArea") => SelectMode::MoveToSelectedArea,
            _ => SelectMode::MoveToCursor,
        };
        if let Some(value) = fields.get("PositionOffsets") {
            let offsets = value
                .as_object()
                .ok_or_else(|| Error::InvalidFormat("Invalid PositionOffsets".into()))?;
            for mount in Mount::ALL {
                if let Some(value) = offsets.get(mount.name()) {
                    let offset = value
                        .as_i64()
                        .and_then(|v| i32::try_from(v).ok())
                        .ok_or_else(|| {
                            Error::InvalidFormat(format!(
                                "Invalid PositionOffsets.{}",
                                mount.name()
                            ))
                        })?;
                    result.set_offset(mount, offset);
                }
            }
        }
        Ok(result)
    }
    pub fn write(self, fields: &mut Map<String, Value>) {
        fields.insert("MiniMapPosition".into(), self.mount.name().into());
        fields.insert(
            "MiniMapSelectMode".into(),
            match self.select_mode {
                SelectMode::MoveToCursor => "MoveToCursor",
                SelectMode::MoveToSelectedArea => "MoveToSelectedArea",
            }
            .into(),
        );
        let mut offsets = fields
            .get("PositionOffsets")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for mount in Mount::ALL {
            offsets.insert(mount.name().into(), self.offset(mount).into());
        }
        fields.insert("PositionOffsets".into(), Value::Object(offsets));
    }
}
