//! GrooveAuthor's dark background preferences; independent of chart history bytes.
use crate::Error;
use serde_json::{Map, Value};
pub(crate) mod history;
pub use history::Update;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Size {
    Charts,
    #[default]
    Window,
    Unknown(i32),
}
impl Size {
    pub fn from_value(value: i32) -> Self {
        match value {
            0 => Self::Charts,
            1 => Self::Window,
            other => Self::Unknown(other),
        }
    }
    pub fn name(self) -> String {
        match self {
            Self::Charts => "Charts".into(),
            Self::Window => "Window".into(),
            Self::Unknown(value) => value.to_string(),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    AfterBackground,
    AfterWaveForm,
    Unknown(i32),
}
impl Order {
    pub fn from_value(value: i32) -> Self {
        match value {
            0 => Self::AfterBackground,
            1 => Self::AfterWaveForm,
            other => Self::Unknown(other),
        }
    }
    pub fn name(self) -> String {
        match self {
            Self::AfterBackground => "AfterBackground".into(),
            Self::AfterWaveForm => "AfterWaveForm".into(),
            Self::Unknown(value) => value.to_string(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    show: bool,
    size: Size,
    order: Order,
    color: [f32; 4],
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            show: false,
            size: Size::Window,
            order: Order::AfterBackground,
            color: [0.0, 0.0, 0.0, 0.8],
        }
    }
}
impl Preferences {
    pub fn show(self) -> bool {
        self.show
    }
    pub fn size(self) -> Size {
        self.size
    }
    pub fn order(self) -> Order {
        self.order
    }
    pub fn color(self) -> [f32; 4] {
        self.color
    }
    /// Apply the exact packed MonoGame Color boundary only at rendering.
    pub fn rendered_color(self) -> [f32; 4] {
        crate::source_color::rgba(crate::source_color::quantize(self.color))
    }
    pub fn set_show(&mut self, value: bool) {
        self.show = value;
    }
    pub fn set_size(&mut self, value: Size) {
        self.size = match value {
            Size::Unknown(value) => Size::from_value(value),
            known => known,
        };
    }
    pub fn set_order(&mut self, value: Order) {
        self.order = match value {
            Order::Unknown(value) => Order::from_value(value),
            known => known,
        };
    }
    pub fn set_color(&mut self, value: [f32; 4]) -> Result<(), Error> {
        if value.iter().any(|value| !value.is_finite()) {
            return Err(Error::InvalidEdit(
                "Dark background components must be finite".into(),
            ));
        }
        self.color = value;
        Ok(())
    }
    pub fn validate(self) -> Result<(), Error> {
        let mut value = self;
        value.set_color(self.color)
    }
    pub(crate) fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let mut value = Self::default();
        if let Some(field) = fields.get("ShowDarkBg") {
            value.show = field
                .as_bool()
                .ok_or_else(|| Error::InvalidFormat("Invalid ShowDarkBg".into()))?;
        }
        if let Some(field) = fields.get("Size") {
            value.size = match field.as_str() {
                Some("Charts") => Size::Charts,
                Some(value) => value
                    .parse::<i32>()
                    .map(Size::from_value)
                    .unwrap_or(Size::Window),
                None => Size::Window,
            };
        }
        if let Some(field) = fields.get("DarkBgDrawOrder") {
            value.order = match field.as_str() {
                Some("AfterWaveForm") => Order::AfterWaveForm,
                Some(value) => value
                    .parse::<i32>()
                    .map(Order::from_value)
                    .unwrap_or(Order::AfterBackground),
                None => Order::AfterBackground,
            };
        }
        if let Some(field) = fields.get("Color") {
            let invalid =
                || Error::InvalidFormat("Dark Color must contain numeric X/Y/Z/W fields".into());
            let object = field.as_object().ok_or_else(invalid)?;
            // System.Numerics.Vector4 deserializes omitted components to zero.
            let mut rgba = [0.0; 4];
            for (target, name) in rgba.iter_mut().zip(["X", "Y", "Z", "W"]) {
                if let Some(component) = object.get(name) {
                    *target = component.as_f64().ok_or_else(invalid)? as f32;
                }
            }
            value.set_color(rgba)?;
        }
        Ok(value)
    }
    pub(crate) fn write(self, fields: &mut Map<String, Value>) {
        fields.insert("ShowDarkBg".into(), self.show.into());
        fields.insert("Size".into(), self.size.name().into());
        fields.insert("DarkBgDrawOrder".into(), self.order.name().into());
        let mut color = fields
            .get("Color")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (name, component) in ["X", "Y", "Z", "W"].into_iter().zip(self.color) {
            color.insert(name.into(), component.into());
        }
        fields.insert("Color".into(), Value::Object(color));
    }
}
