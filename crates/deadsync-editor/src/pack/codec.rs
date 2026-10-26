use super::{Data, Error, MAX_BYTES, bounded, validate};
use std::{ops::Range, sync::Arc};
pub(super) const KEYS: [&str; 7] = [
    "DisplayTitle",
    "TranslitTitle",
    "SortTitle",
    "Series",
    "Year",
    "Banner",
    "SyncOffset",
];
#[derive(Clone, Copy, Debug)]
enum Encoding {
    Utf8,
    Utf16(bool),
    Utf32(bool),
}
impl Encoding {
    fn width(self) -> usize {
        match self {
            Self::Utf8 => 1,
            Self::Utf16(_) => 2,
            Self::Utf32(_) => 4,
        }
    }
    fn encode(self, s: &str) -> Vec<u8> {
        match self {
            Self::Utf8 => s.as_bytes().to_vec(),
            Self::Utf16(le) => s
                .encode_utf16()
                .flat_map(|v| if le { v.to_le_bytes() } else { v.to_be_bytes() })
                .collect(),
            Self::Utf32(le) => s
                .chars()
                .flat_map(|v| {
                    if le {
                        (v as u32).to_le_bytes()
                    } else {
                        (v as u32).to_be_bytes()
                    }
                })
                .collect(),
        }
    }
    fn decode(self, bytes: &[u8]) -> String {
        match self {
            Self::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
            Self::Utf16(le) => String::from_utf16_lossy(
                &bytes
                    .chunks(2)
                    .map(|v| {
                        if v.len() == 2 {
                            if le {
                                u16::from_le_bytes([v[0], v[1]])
                            } else {
                                u16::from_be_bytes([v[0], v[1]])
                            }
                        } else {
                            0xfffd
                        }
                    })
                    .collect::<Vec<_>>(),
            ),
            Self::Utf32(le) => bytes
                .chunks(4)
                .map(|v| {
                    if let Ok(a) = <[u8; 4]>::try_from(v) {
                        char::from_u32(if le {
                            u32::from_le_bytes(a)
                        } else {
                            u32::from_be_bytes(a)
                        })
                        .unwrap_or('\u{fffd}')
                    } else {
                        '\u{fffd}'
                    }
                })
                .collect(),
        }
    }
    fn ascii(self, bytes: &[u8], offset: usize, value: u8) -> bool {
        bytes
            .get(offset..offset + self.width())
            .is_some_and(|s| s == self.encode(&(value as char).to_string()))
    }
}
#[derive(Clone, Debug)]
pub struct Document {
    pub(super) bytes: Arc<[u8]>,
    encoding: Encoding,
    version: i32,
    data: Data,
    fields: [Option<Range<usize>>; 7],
    insert: usize,
    newline: String,
}
impl Document {
    pub fn data(&self) -> &Data {
        &self.data
    }
    pub fn version(&self) -> i32 {
        self.version
    }
    pub fn original_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn parse(bytes: Vec<u8>) -> Result<Self, Error> {
        if bytes.len() > MAX_BYTES {
            return Err(Error("Pack.ini exceeds the metadata size limit".into()));
        }
        let (encoding, bom) = if bytes.starts_with(&[0xff, 0xfe, 0, 0]) {
            (Encoding::Utf32(true), 4)
        } else if bytes.starts_with(&[0, 0, 0xfe, 0xff]) {
            (Encoding::Utf32(false), 4)
        } else if bytes.starts_with(&[0xff, 0xfe]) {
            (Encoding::Utf16(true), 2)
        } else if bytes.starts_with(&[0xfe, 0xff]) {
            (Encoding::Utf16(false), 2)
        } else if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
            (Encoding::Utf8, 3)
        } else {
            (Encoding::Utf8, 0)
        };
        let width = encoding.width();
        let mut pos = bom;
        let mut found = false;
        let mut active = false;
        let mut insert = bytes.len();
        let mut version = None;
        let mut fields: [Option<Range<usize>>; 7] = std::array::from_fn(|_| None);
        let mut values: [Option<String>; 7] = std::array::from_fn(|_| None);
        let mut newline = None;
        while pos < bytes.len() {
            let start = pos;
            while pos < bytes.len()
                && !encoding.ascii(&bytes, pos, b'\r')
                && !encoding.ascii(&bytes, pos, b'\n')
            {
                pos = (pos + width).min(bytes.len());
            }
            let end = pos;
            if pos < bytes.len() {
                let cr = encoding.ascii(&bytes, pos, b'\r');
                pos += width;
                if cr && encoding.ascii(&bytes, pos, b'\n') {
                    pos += width;
                }
                newline.get_or_insert_with(|| encoding.decode(&bytes[end..pos]));
            }
            let line = encoding.decode(&bytes[start..end]);
            if line.starts_with('[') && line.ends_with(']') {
                if active {
                    insert = start;
                    active = false;
                }
                if !found && line == "[Group]" {
                    found = true;
                    active = true;
                }
                continue;
            }
            if !active || [";", "#", "//", "--"].iter().any(|v| line.starts_with(v)) {
                continue;
            }
            let equals = (start..end)
                .step_by(width)
                .find(|&p| encoding.ascii(&bytes, p, b'='));
            let Some(eq) = equals else { continue };
            let key = encoding.decode(&bytes[start..eq]);
            let value = encoding.decode(&bytes[eq + width..end]);
            if key.trim() == "Version" && version.is_none() {
                version = Some(value.clone());
            }
            if let Some(index) = KEYS.iter().position(|k| *k == key.trim()) {
                if fields[index].is_none() {
                    fields[index] = Some(eq + width..end);
                    values[index] = Some(value);
                }
            }
        }
        let version = version
            .and_then(|v| v.trim().parse::<i32>().ok())
            .ok_or_else(|| Error("Pack.ini requires a valid Group Version".into()))?;
        let sync = values[6].as_deref().map(parse_sync).unwrap_or(0);
        let data = Data {
            title: values[0].take(),
            title_transliteration: values[1].take(),
            title_sort: values[2].take(),
            series: values[3].take(),
            year: values[4]
                .as_deref()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0),
            banner: values[5].take(),
            sync_offset: sync,
        };
        Ok(Self {
            bytes: bytes.into(),
            encoding,
            version,
            data,
            fields,
            insert,
            newline: newline.unwrap_or_else(|| "\n".into()),
        })
    }
    pub fn render(&self, data: &Data) -> Result<Vec<u8>, Error> {
        if data == &self.data {
            return Ok(self.bytes.to_vec());
        }
        validate(data)?;
        let before = self.data.values();
        let after = data.values();
        let mut patches = Vec::new();
        let mut added = String::new();
        for i in 0..7 {
            if before[i] == after[i] {
                continue;
            }
            let value = after[i].as_deref().unwrap_or("");
            if let Some(range) = &self.fields[i] {
                patches.push((range.clone(), self.encoding.encode(value)));
            } else {
                added.push_str(KEYS[i]);
                added.push('=');
                added.push_str(value);
                added.push_str(&self.newline);
            }
        }
        if !added.is_empty() {
            let prefix = self.encoding.decode(&self.bytes[..self.insert]);
            if !prefix.ends_with(['\r', '\n']) {
                added.insert_str(0, &self.newline);
            }
            patches.push((self.insert..self.insert, self.encoding.encode(&added)));
        }
        patches.sort_by_key(|(r, _)| r.start);
        let mut output = Vec::new();
        let mut cursor = 0;
        for (range, replacement) in patches {
            output.extend_from_slice(&self.bytes[cursor..range.start]);
            output.extend(replacement);
            cursor = range.end;
        }
        output.extend_from_slice(&self.bytes[cursor..]);
        let output = bounded(output)?;
        if Self::parse(output.clone())?.data != *data {
            return Err(Error(
                "Pack edit cannot be represented without changing metadata".into(),
            ));
        }
        Ok(output)
    }
}
fn parse_sync(value: &str) -> i32 {
    let value = value.trim();
    if let Ok(v) = value.parse::<i32>() {
        return v;
    }
    value
        .split(',')
        .try_fold(0, |v, s| match s.trim() {
            "NULL" => Some(v),
            "ITG" => Some(v | 1),
            _ => None,
        })
        .unwrap_or(0)
}
