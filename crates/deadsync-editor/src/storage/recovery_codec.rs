//! Recovery is a lossless native snapshot, never an advanced export. V2 records
//! the source codec and target policy independently; V1 remains readable.
use super::*;
use crate::{generation::metadata::Guid, output_policy::OutputPolicy};
use serde_json::{Map, Value};
const V1: &[u8] = b"DEADSYNC-EDITOR-RECOVERY-1\n";
const V2: &[u8] = b"DEADSYNC-EDITOR-RECOVERY-2\n";
const MAX_HEADER: usize = 16 * 1024 * 1024;
const MAX_PAYLOAD:usize=256*1024*1024;
pub(super) struct Decoded {
    pub session: Session,
    pub policy: OutputPolicy,
}
fn invalid(message: &str) -> Error {
    Error::InvalidFormat(message.into())
}
fn format(value: &Value) -> Result<Format, Error> {
    match value.as_str() {
        Some("sm") => Ok(Format::Sm),
        Some("ssc") => Ok(Format::Ssc),
        _ => Err(invalid("Invalid native recovery codec")),
    }
}
fn policy_value(policy: &OutputPolicy) -> Value {
    match policy {
        OutputPolicy::NativeLossless => serde_json::json!({"kind":"native_lossless"}),
        OutputPolicy::Advanced {
            format,
            options,
            timing_chart,
        } => {
            let mut fields = Map::new();
            options.write(&mut fields);
            serde_json::json!({"kind":"advanced","format":format.extension(),"options":fields,"timing_chart":timing_chart.to_string()})
        }
    }
}
fn read_policy(value: &Value) -> Result<OutputPolicy, Error> {
    let fields = value
        .as_object()
        .ok_or_else(|| invalid("Invalid recovery output policy"))?;
    match fields.get("kind").and_then(Value::as_str) {
        Some("native_lossless") => Ok(OutputPolicy::NativeLossless),
        Some("advanced") => {
            let options = fields
                .get("options")
                .and_then(Value::as_object)
                .ok_or_else(|| invalid("Incomplete recovery output options"))?;
            if options.len() != crate::advanced_export::Options::NAMES.len() {
                return Err(invalid("Unsupported recovery output options"));
            }
            if crate::advanced_export::Options::NAMES
                .iter()
                .any(|name| !options.contains_key(*name))
            {
                return Err(invalid("Incomplete recovery output options"));
            }
            Ok(OutputPolicy::Advanced {
                format: format(
                    fields
                        .get("format")
                        .ok_or_else(|| invalid("Missing recovery output format"))?,
                )?,
                options: crate::advanced_export::Options::read(options)?,
                timing_chart: fields
                    .get("timing_chart")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("Missing recovery timing identity"))?
                    .parse()?,
            })
        }
        _ => Err(invalid("Unknown recovery output policy")),
    }
}
fn path_value(path: &Path) -> Result<Value, Error> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(serde_json::json!({"platform":"unix","value":path.as_os_str().as_bytes()}))
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        Ok(
            serde_json::json!({"platform":"windows","value":path.as_os_str().encode_wide().collect::<Vec<_>>()}),
        )
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(
            serde_json::json!({"platform":"unicode","value":path.to_str().ok_or_else(||invalid("Recovery target path cannot be encoded"))?}),
        )
    }
}
pub(super) fn encode(snapshot: &RecoverySnapshot) -> Result<Vec<u8>, Error> {
    if snapshot.content.len()>MAX_PAYLOAD{return Err(invalid("Native recovery payload limit exceeded"));}
    let header = serde_json::json!({"version":2,"source_format":snapshot.source_format.extension(),"policy":policy_value(&snapshot.policy),"target":path_value(&snapshot.source)?,"chart_ids":snapshot.chart_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),"expected_length":snapshot.expected.len(),"content_length":snapshot.content.len()});
    let header =
        serde_json::to_vec(&header).map_err(|error| Error::InvalidFormat(error.to_string()))?;
    if header.len() > MAX_HEADER {
        return Err(invalid("Recovery header limit exceeded"));
    }
    let total = V2
        .len()
        .checked_add(8)
        .and_then(|size| size.checked_add(header.len()))
        .and_then(|size| size.checked_add(snapshot.expected.len()))
        .and_then(|size| size.checked_add(snapshot.content.len()))
        .ok_or_else(|| invalid("Recovery size overflow"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| invalid("Recovery snapshot allocation failed"))?;
    bytes.extend_from_slice(V2);
    bytes.extend_from_slice(&(header.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(&snapshot.expected);
    bytes.extend_from_slice(&snapshot.content);
    Ok(bytes)
}
fn length(bytes: &[u8], start: usize) -> Result<(usize, usize), Error> {
    let end = start
        .checked_add(8)
        .ok_or_else(|| invalid("Recovery length overflow"))?;
    let value = bytes
        .get(start..end)
        .ok_or_else(|| invalid("Incomplete recovery length"))?;
    let value = usize::try_from(u64::from_le_bytes(
        value
            .try_into()
            .map_err(|_| invalid("Invalid recovery length"))?,
    ))
    .map_err(|_| invalid("Recovery length overflow"))?;
    Ok((value, end))
}
pub(super) fn decode(
    bytes: &[u8],
    expected: &[u8],
    target: &Path,
    legacy_format: Format,
) -> Result<Decoded, Error> {
    if bytes.starts_with(V1) {
        let (size, start) = length(bytes, V1.len())?;
        let end = start
            .checked_add(size)
            .ok_or_else(|| invalid("Recovery length overflow"))?;
        if bytes.get(start..end) != Some(expected) {
            return Err(Error::ExternalChange);
        }
        let content = bytes
            .get(end..)
            .ok_or_else(|| invalid("Incomplete recovery payload"))?;
        return Ok(Decoded {
            session: Session::recovered(Document::parse(
                Arc::<[u8]>::from(content),
                legacy_format,
            )?),
            policy: OutputPolicy::NativeLossless,
        });
    }
    if !bytes.starts_with(V2) {
        return Err(invalid("Invalid recovery header"));
    }
    let (size, start) = length(bytes, V2.len())?;
    if size > MAX_HEADER {
        return Err(invalid("Recovery header limit exceeded"));
    }
    let end = start
        .checked_add(size)
        .ok_or_else(|| invalid("Recovery length overflow"))?;
    let header: Value = serde_json::from_slice(
        bytes
            .get(start..end)
            .ok_or_else(|| invalid("Incomplete recovery header"))?,
    )
    .map_err(|error| Error::InvalidFormat(error.to_string()))?;
    let header = header
        .as_object()
        .ok_or_else(|| invalid("Invalid recovery metadata"))?;
    if header.get("version").and_then(Value::as_u64) != Some(2) {
        return Err(invalid("Unsupported recovery version"));
    }
    if header.get("target") != Some(&path_value(target)?) {
        return Err(Error::ExternalChange);
    }
    let size = usize::try_from(
        header
            .get("expected_length")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("Invalid recovery baseline length"))?,
    )
    .map_err(|_| invalid("Recovery baseline overflow"))?;
    let baseline_end = end
        .checked_add(size)
        .ok_or_else(|| invalid("Recovery baseline overflow"))?;
    if bytes.get(end..baseline_end) != Some(expected) {
        return Err(Error::ExternalChange);
    }
    let source_format = format(
        header
            .get("source_format")
            .ok_or_else(|| invalid("Missing native recovery codec"))?,
    )?;
    let policy = read_policy(
        header
            .get("policy")
            .ok_or_else(|| invalid("Missing recovery output policy"))?,
    )?;
    super::save_output::validate_extension(
        target,
        super::save_output::policy_format(&policy, source_format),
    )?;
    let identities = header
        .get("chart_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Missing recovery chart identities"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| invalid("Invalid recovery chart identity"))?
                .parse::<Guid>()
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let content_length=usize::try_from(header.get("content_length").and_then(Value::as_u64).ok_or_else(||invalid("Missing native recovery payload length"))?).map_err(|_|invalid("Native recovery payload length overflow"))?;
    if content_length>MAX_PAYLOAD{return Err(invalid("Native recovery payload limit exceeded"));}
    let content_end=baseline_end.checked_add(content_length).ok_or_else(||invalid("Native recovery payload length overflow"))?;
    if content_end!=bytes.len(){return Err(invalid("Native recovery payload length mismatch"));}
    let content=bytes.get(baseline_end..content_end).ok_or_else(||invalid("Incomplete native recovery payload"))?;
    let session = Session::recovered_output_snapshot(
        Document::parse(Arc::<[u8]>::from(content), source_format)?,
        identities,
    )?;
    policy.timing_chart(&session)?;
    Ok(Decoded { session, policy })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file() -> (tempfile::TempDir, FileSession) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source.ssc");
        fs::write(&path,"#VERSION:0.83;#TITLE:Policy;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
        let file = FileSession::open(&path).unwrap();
        (root, file)
    }
    fn changed_header(bytes: &[u8], change: impl FnOnce(&mut Value)) -> Vec<u8> {
        let (size, start) = length(bytes, V2.len()).unwrap();
        let end = start + size;
        let mut value: Value = serde_json::from_slice(&bytes[start..end]).unwrap();
        change(&mut value);
        let header = serde_json::to_vec(&value).unwrap();
        let mut output = V2.to_vec();
        output.extend_from_slice(&(header.len() as u64).to_le_bytes());
        output.extend_from_slice(&header);
        output.extend_from_slice(&bytes[end..]);
        output
    }
    #[test]
    fn invalid_v2_identity_lists_codecs_and_lengths_are_rejected_without_live_mutation() {
        let (_root, mut file) = file();
        let revision = file.session.revision();
        file.session.duplicate_chart(0, revision).unwrap();
        let snapshot = file.recovery_snapshot().unwrap().unwrap();
        let bytes = encode(&snapshot).unwrap();
        let expected = file.expected.as_ref().unwrap();
        let path = file.require_existing_path().unwrap();
        let before = file.session.serialize().unwrap();
        let mutations: Vec<Box<dyn FnOnce(&mut Value)>> = vec![
            Box::new(|value| value["chart_ids"] = serde_json::json!([])),
            Box::new(|value| {
                value["chart_ids"] = serde_json::json!([
                    "00000000-0000-0000-0000-000000000000",
                    "00000000-0000-0000-0000-000000000000"
                ])
            }),
            Box::new(|value| value["source_format"] = serde_json::json!("invalid")),
            Box::new(|value| value["version"] = serde_json::json!(3)),
            Box::new(|value| value["expected_length"] = serde_json::json!(u64::MAX)),
            Box::new(|value| value["target"] = serde_json::json!("another file")),
        ];
        for mutation in mutations {
            let altered = changed_header(&bytes, mutation);
            assert!(decode(&altered, expected, path, Format::Ssc).is_err());
            assert_eq!(file.session.serialize().unwrap(), before);
        }
        let (header_length, header_start) = length(&bytes, V2.len()).unwrap();
        let header_end = header_start + header_length;
        let baseline_end = header_end + expected.len();
        assert!(decode(&bytes, expected, path, Format::Ssc).is_ok());
        for cut in [
            0,
            1,
            V2.len(),
            V2.len() + 7,
            header_end - 1,
            baseline_end - 1,
        ] {
            assert!(decode(&bytes[..cut], expected, path, Format::Ssc).is_err());
        }
    }
    #[test]
    fn missing_advanced_timing_identity_is_rejected() {
        let (_root, file) = file();
        let snapshot = file.recovery_snapshot().unwrap().unwrap();
        let bytes = encode(&snapshot).unwrap();
        let altered = changed_header(&bytes, |value| {
            value["policy"] = policy_value(&OutputPolicy::Advanced {
                format: Format::Ssc,
                options: Default::default(),
                timing_chart: Guid::random(),
            });
        });
        assert!(
            decode(
                &altered,
                file.expected.as_ref().unwrap(),
                file.require_existing_path().unwrap(),
                Format::Ssc
            )
            .is_err()
        );
    }
    #[test]
    #[cfg(unix)]
    fn non_unicode_target_path_is_preserved_without_lossy_conversion() {
        use std::os::unix::ffi::{OsStrExt,OsStringExt};
        // macOS filesystems can reject non-UTF8 names. Exercise the codec with
        // exact Unix path bytes without asking the filesystem to create one.
        let(root,file)=file();
        let path=root.path().join(std::ffi::OsString::from_vec(b"song-\xff.ssc".to_vec()));
        let mut snapshot=file.recovery_snapshot().unwrap().unwrap();
        snapshot.source=path.clone();
        let bytes=encode(&snapshot).unwrap();
        let expected=file.expected.as_ref().unwrap();
        let decoded=decode(&bytes,expected,&path,Format::Ssc).unwrap();
        assert_eq!(decoded.session.serialize().unwrap(),snapshot.content);
        let(size,start)=length(&bytes,V2.len()).unwrap();
        let header:Value=serde_json::from_slice(&bytes[start..start+size]).unwrap();
        let encoded=header["target"]["value"].as_array().unwrap().iter().map(|v|u8::try_from(v.as_u64().unwrap()).unwrap()).collect::<Vec<_>>();
        assert_eq!(encoded,path.as_os_str().as_bytes());
        assert!(encoded.contains(&0xff));
        let lossy=root.path().join("song-\u{fffd}.ssc");
        assert!(matches!(decode(&bytes,expected,&lossy,Format::Ssc),Err(Error::ExternalChange)));
    }
    #[test]
    fn v2_exact_frame_rejects_parseable_native_prefix_and_extra_bytes() {
        let root=tempfile::tempdir().unwrap();let path=root.path().join("empty.ssc");fs::write(&path,b"#VERSION:0.83;#TITLE:Complete source;#CREDIT:Preserve author;").unwrap();let mut file=FileSession::open(&path).unwrap();file.session.set_text(None,"TITLE","Complete native draft").unwrap();let snapshot=file.recovery_snapshot().unwrap().unwrap();let bytes=encode(&snapshot).unwrap();let expected=file.expected.as_ref().unwrap();let path=file.require_existing_path().unwrap();let before=file.session.serialize().unwrap();
        let(size,start)=length(&bytes,V2.len()).unwrap();let payload_start=start+size+expected.len();let prefix=b"#VERSION:0.83;";assert!(Document::parse(Arc::<[u8]>::from(prefix.as_slice()),Format::Ssc).is_ok());let mut partial=bytes[..payload_start].to_vec();partial.extend_from_slice(prefix);assert!(decode(&partial,expected,path,Format::Ssc).is_err());
        let mut extra=bytes.clone();extra.extend_from_slice(b"#SUBTITLE:Parseable extra;");assert!(decode(&extra,expected,path,Format::Ssc).is_err());
        for mutation in [0,u64::MAX,(MAX_PAYLOAD as u64)+1]{let changed=changed_header(&bytes,|header|header["content_length"]=serde_json::json!(mutation));assert!(decode(&changed,expected,path,Format::Ssc).is_err());}
        let missing=changed_header(&bytes,|header|{header.as_object_mut().unwrap().remove("content_length");});assert!(decode(&missing,expected,path,Format::Ssc).is_err());assert!(decode(&bytes,expected,path,Format::Ssc).is_ok());assert_eq!(file.session.serialize().unwrap(),before);
    }
    #[test]
    fn v1_legacy_payload_remains_readable_without_v2_length_metadata() {
        let(_root,file)=file();let expected=file.expected.as_ref().unwrap();let content=file.session.serialize().unwrap();let mut bytes=V1.to_vec();bytes.extend_from_slice(&(expected.len()as u64).to_le_bytes());bytes.extend_from_slice(expected);bytes.extend_from_slice(&content);let decoded=decode(&bytes,expected,file.require_existing_path().unwrap(),Format::Ssc).unwrap();assert_eq!(decoded.session.serialize().unwrap(),content);assert!(matches!(decoded.policy,OutputPolicy::NativeLossless));
    }

}
