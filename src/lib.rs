//! Reads Figma `.fig` files (File > Save local copy) without Figma.
//!
//! `canvas.fig` carries its own kiwi schema, so decoding needs no hardcoded
//! knowledge of Figma's fields; only the container framing is fixed.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use kiwi_schema::{Schema, Value};

pub mod bundle;
pub mod clipboard;
pub mod compare;
pub mod compare_image;
pub mod expand;
pub mod export;
pub mod fonts;
pub mod freshness;
pub mod images;
pub mod output;
mod parse;
pub mod preview;
mod prototype;
pub mod remote;
pub mod render;
pub mod rest;
pub mod search;
pub mod semantic;
pub mod states;
pub mod tokens;
pub mod variables;
pub mod verify;
pub mod workspace;

#[cfg(test)]
mod test_support;

const MAGIC: &[u8; 8] = b"fig-kiwi";
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidData,
    InvalidArguments,
    SourceUnreadable,
    NodeNotFound,
    UnsupportedNode,
    ParseBudget,
    ResponseBudget,
    OutputWrite,
    PermissionDenied,
    OperationFailed,
    WorkspaceBusy,
    IndexIncompatible,
    StorageUnavailable,
    /// A source changed on disk since its index was verified.
    SourceStale,
    /// A cursor's sources were rebuilt since it was issued.
    CursorStale,
    /// A cursor's generation no longer exists.
    CursorExpired,
    /// Retention deleted revisions after a change cursor or checkpoint before they were read.
    EventGap,
    Cancelled,
    /// A workspace search would read more indexed data than one query supports.
    QueryWorkLimit,
    /// The requested search mode has no usable model profile or layer.
    ModeUnavailable,
}

#[derive(Debug)]
pub struct FigError {
    message: String,
    kind: ErrorKind,
    /// Recovery actions that are implemented and apply to this failure, for agents to run.
    recovery: Option<serde_json::Value>,
}

impl FigError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            recovery: None,
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Attaches the actions that recover from this failure: `[{action, tool?, arguments?,
    /// requires: none|user_confirmation, ...}]`.
    pub fn with_recovery(mut self, recovery: serde_json::Value) -> Self {
        self.recovery = Some(recovery);
        self
    }

    pub fn recovery(&self) -> Option<&serde_json::Value> {
        self.recovery.as_ref()
    }
}

impl fmt::Display for FigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for FigError {}

fn err<T>(message: impl Into<String>) -> Result<T, Box<dyn Error>> {
    Err(Box::new(FigError::new(
        ErrorKind::InvalidData,
        message.into(),
    )))
}

pub(crate) fn open_zip(path: &Path) -> Result<zip::ZipArchive<BufReader<File>>, Box<dyn Error>> {
    // Avoid Detect's full-file scan; retain it for archives with prepended bytes.
    let config = zip::read::Config {
        archive_offset: zip::read::ArchiveOffset::Known(0),
    };
    match zip::ZipArchive::with_config(config, BufReader::new(File::open(path)?)) {
        Ok(archive) => Ok(archive),
        Err(_) => Ok(zip::ZipArchive::new(BufReader::new(File::open(path)?))?),
    }
}

/// Open immutable staged bytes with the same offset fast path and fallback as a file.
pub(crate) fn open_zip_bytes(
    bytes: &[u8],
) -> Result<zip::ZipArchive<std::io::Cursor<&[u8]>>, zip::result::ZipError> {
    let config = zip::read::Config {
        archive_offset: zip::read::ArchiveOffset::Known(0),
    };
    zip::ZipArchive::with_config(config, std::io::Cursor::new(bytes))
        .or_else(|_| zip::ZipArchive::new(std::io::Cursor::new(bytes)))
}

/// A decoded `canvas.fig`: the embedded schema plus the raw message bytes.
pub struct Document {
    pub version: u32,
    pub schema: Schema,
    message: Vec<u8>,
}

impl Document {
    /// Opens a `.fig` zip and decodes its `canvas.fig`.
    pub fn open(path: &Path) -> Result<Document, Box<dyn Error>> {
        Self::from_archive(open_zip(path)?, &path.display().to_string())
    }

    /// `open` for `.fig` bytes already in memory, so a caller can hash exactly what it decodes.
    pub fn from_bytes(fig: &[u8], label: &str) -> Result<Document, Box<dyn Error>> {
        let archive = open_zip_bytes(fig).map_err(|e| {
            FigError::new(
                ErrorKind::InvalidData,
                format!("{label}: not a .fig zip ({e})"),
            )
        })?;
        Self::from_archive(archive, label)
    }

    fn from_archive<R: Read + std::io::Seek>(
        mut archive: zip::ZipArchive<R>,
        label: &str,
    ) -> Result<Document, Box<dyn Error>> {
        let canvas_file = archive.by_name("canvas.fig").map_err(|e| {
            FigError::new(
                ErrorKind::InvalidData,
                format!("{label}: no canvas.fig ({e})"),
            )
        })?;
        if canvas_file.size() > parse::CANVAS_BYTES {
            return Err(parse::exceeded("canvas bytes", parse::CANVAS_BYTES).into());
        }
        let canvas = read_limited(canvas_file, parse::CANVAS_BYTES, "canvas bytes")?;
        Document::from_canvas(&canvas)
    }

    pub fn from_canvas(canvas: &[u8]) -> Result<Document, Box<dyn Error>> {
        let (version, chunks) = split_chunks(canvas)?;
        if chunks.len() < 2 {
            return err(format!(
                "canvas.fig has {} chunk(s), expected schema and message",
                chunks.len()
            ));
        }
        let schema_bytes = unpack_limited(chunks[0], parse::SCHEMA_BYTES, "schema bytes")?;
        parse::schema(&schema_bytes)?;
        let schema = Schema::decode(&schema_bytes)
            .map_err(|_| FigError::new(ErrorKind::InvalidData, "invalid kiwi schema"))?;
        Ok(Document {
            version,
            schema,
            message: unpack_limited(chunks[1], parse::MESSAGE_BYTES, "message bytes")?,
        })
    }

    /// Decodes the root `Message` with the file's own schema, retaining bytes for another decode.
    pub fn message(&self) -> Result<Value<'_>, Box<dyn Error>> {
        Self::decode_message(&self.schema, &self.message)
    }

    /// Decodes the root `Message` and releases its bytes; the returned value borrows only the schema.
    pub fn take_message(&mut self) -> Result<Value<'_>, Box<dyn Error>> {
        let message = std::mem::take(&mut self.message);
        Self::decode_message(&self.schema, &message)
    }

    fn decode_message<'a>(schema: &'a Schema, message: &[u8]) -> Result<Value<'a>, Box<dyn Error>> {
        let Some(def) = schema.def("Message") else {
            return err("schema has no Message definition");
        };
        parse::message(schema, def.index, message, parse::VALUES)?;
        Value::decode(schema, def.index, message).map_err(|_| {
            FigError::new(ErrorKind::InvalidData, "message does not match its schema").into()
        })
    }
}

/// Format version and the chunk slices of a `canvas.fig`.
pub type Chunks<'a> = (u32, Vec<&'a [u8]>);

/// Splits `fig-kiwi` framing: magic, u32 version, then u32-length-prefixed chunks (all little-endian).
pub fn split_chunks(data: &[u8]) -> Result<Chunks<'_>, Box<dyn Error>> {
    if data.len() as u64 > parse::CANVAS_BYTES {
        return Err(parse::exceeded("canvas bytes", parse::CANVAS_BYTES).into());
    }
    if data.len() < 12 || &data[..8] != MAGIC {
        return err("not a fig-kiwi canvas");
    }
    let version = u32::from_le_bytes(data[8..12].try_into()?);
    let mut chunks = Vec::new();
    let mut offset: usize = 12;
    while offset < data.len() {
        if chunks.len() >= parse::CHUNKS {
            return Err(parse::exceeded("chunk count", parse::CHUNKS).into());
        }
        let header_end = offset
            .checked_add(4)
            .ok_or_else(|| FigError::new(ErrorKind::InvalidData, "chunk offset overflow"))?;
        let Some(header) = data.get(offset..header_end) else {
            return err(format!("truncated chunk header at byte {offset}"));
        };
        let len = u32::from_le_bytes(header.try_into()?) as usize;
        let end = header_end
            .checked_add(len)
            .ok_or_else(|| FigError::new(ErrorKind::InvalidData, "chunk length overflow"))?;
        let Some(chunk) = data.get(header_end..end) else {
            return err(format!(
                "chunk at byte {offset} claims {len} bytes past the end"
            ));
        };
        chunks.push(chunk);
        offset = end;
    }
    Ok((version, chunks))
}

/// Chunks are zstd (newer files) or raw deflate (older files).
pub fn unpack(chunk: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    unpack_limited(chunk, parse::MESSAGE_BYTES, "decompressed bytes")
}

fn unpack_limited(chunk: &[u8], limit: u64, kind: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    if chunk.starts_with(&ZSTD_MAGIC) {
        read_limited(ruzstd::decoding::StreamingDecoder::new(chunk)?, limit, kind)
    } else {
        read_limited(flate2::read::DeflateDecoder::new(chunk), limit, kind)
    }
}

fn read_limited(reader: impl Read, limit: u64, kind: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut out = Vec::new();
    reader.take(limit.saturating_add(1)).read_to_end(&mut out)?;
    if out.len() as u64 > limit {
        return Err(parse::exceeded(kind, limit).into());
    }
    Ok(out)
}

/// Bound both the advertised and actual image bytes before retaining them in a scope.
pub(crate) fn read_image(
    reader: impl Read,
    size: u64,
    remaining: &mut u64,
) -> Result<Vec<u8>, Box<dyn Error>> {
    if size > parse::IMAGE_BYTES {
        return Err(parse::exceeded("image bytes", parse::IMAGE_BYTES).into());
    }
    if size > *remaining {
        return Err(parse::exceeded("total image bytes", parse::IMAGE_TOTAL_BYTES).into());
    }
    let (limit, kind) = if *remaining < parse::IMAGE_BYTES {
        (*remaining, "remaining image bytes")
    } else {
        (parse::IMAGE_BYTES, "image bytes")
    };
    let bytes = read_limited(reader, limit, kind)?;
    *remaining -= bytes.len() as u64;
    Ok(bytes)
}

/// `12-345` (URL form), `12:345`, or a Figma URL with `node-id=` → `12:345`.
pub fn normalize_id(id: &str) -> String {
    let id = id
        .split_once("node-id=")
        .map_or(id, |(_, rest)| rest.split('&').next().unwrap_or(rest));
    id.replace("%3A", ":").replace('-', ":")
}

/// `sessionID:localID` of an unsigned GUID object, the same id Figma shows in URLs.
pub fn guid_key(guid: &Value) -> Option<String> {
    let (Value::UInt(s), Value::UInt(l)) = (guid.get("sessionID")?, guid.get("localID")?) else {
        return None;
    };
    // Figma writes u32::MAX:u32::MAX for "no guid" (e.g. an unset overrideKey); taking it
    // as a key gives unrelated nodes one id, and the tree then contains itself.
    (*s != u32::MAX || *l != u32::MAX).then(|| format!("{s}:{l}"))
}

/// Node changes indexed by id, with children ordered as Figma orders them.
pub struct Tree<'a> {
    pub nodes: HashMap<String, &'a Value<'a>>,
    pub children: HashMap<String, Vec<String>>,
    /// The message's byte blobs (path commands of geometry and glyphs), by index.
    pub blobs: Vec<&'a Value<'a>>,
}

impl<'a> Tree<'a> {
    /// Bytes of blob `index`, as kiwi `commandsBlob` fields name them.
    pub fn blob(&self, index: u64) -> Option<Vec<u8>> {
        match self.blobs.get(index as usize)?.get("bytes")? {
            Value::Bytes(bytes) => Some(bytes.clone()),
            Value::Array(bytes) => bytes
                .iter()
                .map(|b| match b {
                    Value::Byte(b) => Some(*b),
                    _ => None,
                })
                .collect(),
            _ => None,
        }
    }

    pub fn new(message: &'a Value<'a>) -> Tree<'a> {
        let blobs = match message.get("blobs") {
            Some(Value::Array(blobs)) => blobs.iter().collect(),
            _ => Vec::new(),
        };
        let mut nodes = HashMap::new();
        // One parent per id, from the same record `nodes` keeps (the last one with that guid):
        // `children` and the loop check below must agree on it.
        let mut parents: HashMap<String, (String, String)> = HashMap::new();
        if let Some(Value::Array(changes)) = message.get("nodeChanges") {
            for node in changes {
                let Some(id) = node.get("guid").and_then(guid_key) else {
                    continue;
                };
                let parent = node.get("parentIndex").and_then(|parent| {
                    let position = parent
                        .get("position")
                        .map(|p| p.as_string().to_owned())
                        .unwrap_or_default();
                    Some((parent.get("guid").and_then(guid_key)?, position))
                });
                match parent {
                    Some(parent) => parents.insert(id.clone(), parent),
                    None => parents.remove(&id),
                };
                nodes.insert(id, node);
            }
        }
        // A crafted file can make a node its own ancestor. Drop every node whose parent chain
        // loops (and what hangs under it), so every walk up or down the tree ends.
        let dropped: std::collections::HashSet<String> = {
            // true: the chain ends. A node is false while its own chain is being walked, so
            // meeting it again on that walk is a loop.
            let mut ends: HashMap<&str, bool> = HashMap::with_capacity(nodes.len());
            for start in nodes.keys() {
                let mut path = Vec::new();
                let mut cur = Some(start.as_str());
                let result = loop {
                    let Some(c) = cur else { break true };
                    if let Some(&known) = ends.get(c) {
                        break known;
                    }
                    ends.insert(c, false);
                    path.push(c);
                    cur = parents.get(c).map(|(p, _)| p.as_str());
                };
                if result {
                    ends.extend(path.into_iter().map(|c| (c, true)));
                }
            }
            ends.into_iter()
                .filter(|(_, ok)| !ok)
                .map(|(id, _)| id.to_owned())
                .collect()
        };
        // Real files have no loops: skip the filtering then.
        let keep = |id: &String| dropped.is_empty() || !dropped.contains(id);
        nodes.retain(|id, _| keep(id));
        let mut children: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for (id, (parent, position)) in parents {
            if keep(&id) {
                children.entry(parent).or_default().push((position, id));
            }
        }
        let children = children
            .into_iter()
            .map(|(parent, mut list)| {
                list.sort();
                (parent, list.into_iter().map(|(_, id)| id).collect())
            })
            .collect();
        Tree {
            nodes,
            children,
            blobs,
        }
    }
}

/// Converts a kiwi value to JSON; byte arrays are summarized instead of listed.
pub fn to_json(value: &Value) -> serde_json::Value {
    use serde_json::Value as J;
    match value {
        Value::Bool(v) => J::from(*v),
        Value::Byte(v) => J::from(*v),
        Value::Int(v) => J::from(*v),
        Value::UInt(v) => J::from(*v),
        Value::Float(v) => J::from(*v),
        Value::String(v) => J::from(v.as_str()),
        Value::Int64(v) => J::from(*v),
        Value::UInt64(v) => J::from(*v),
        Value::Enum(_, name) => J::from(*name),
        Value::Bytes(bytes) if bytes.is_empty() => J::Array(Vec::new()),
        Value::Bytes(bytes) => J::from(format!("<{} bytes>", bytes.len())),
        Value::Array(items)
            if !items.is_empty() && items.iter().all(|i| matches!(i, Value::Byte(_))) =>
        {
            J::from(format!("<{} bytes>", items.len()))
        }
        Value::Array(items) => J::Array(items.iter().map(to_json).collect()),
        Value::Object(_, fields) => {
            let mut sorted: Vec<_> = fields.iter().collect();
            sorted.sort_by_key(|(name, _)| *name);
            J::Object(
                sorted
                    .into_iter()
                    .map(|(name, v)| match v {
                        // Image hashes are what REST calls imageRef.
                        Value::Bytes(bytes) if *name == "hash" => (
                            name.to_string(),
                            J::from(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()),
                        ),
                        Value::Array(bytes) if *name == "hash" => (
                            name.to_string(),
                            J::from(
                                bytes
                                    .iter()
                                    .map(|b| match b {
                                        Value::Byte(b) => format!("{b:02x}"),
                                        _ => String::new(),
                                    })
                                    .collect::<String>(),
                            ),
                        ),
                        _ => (name.to_string(), to_json(v)),
                    })
                    .collect(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn opens_normal_and_prefixed_fig_archives() {
        let fixture = Path::new("examples/first-success/design.fig");
        let normal = Document::open(fixture).unwrap();
        let mut canvas = Vec::new();
        open_zip(fixture)
            .unwrap()
            .by_name("canvas.fig")
            .unwrap()
            .read_to_end(&mut canvas)
            .unwrap();
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file("canvas.fig", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&canvas).unwrap();
        let mut prefixed = b"junk".to_vec();
        prefixed.extend(zip.finish().unwrap().into_inner());
        let dir = crate::test_support::TestDir::new();
        let path = dir.join("prefixed.fig");
        std::fs::write(&path, prefixed).unwrap();
        assert_eq!(open_zip(&path).unwrap().offset(), 4);
        let recovered = Document::open(&path).unwrap();
        assert_eq!(recovered.version, normal.version);
        assert_eq!(
            to_json(&recovered.message().unwrap()),
            to_json(&normal.message().unwrap())
        );
        let in_memory =
            Document::from_bytes(&std::fs::read(&path).unwrap(), "prefixed.fig").unwrap();
        assert_eq!(
            to_json(&in_memory.message().unwrap()),
            to_json(&normal.message().unwrap())
        );
        let error = Document::from_bytes(b"not a zip", "broken.fig")
            .err()
            .unwrap();
        assert!(
            error.to_string().starts_with("broken.fig: not a .fig zip"),
            "{error}"
        );
    }

    #[test]
    fn taking_message_releases_bytes_and_preserves_decoded_values() {
        let mut doc = Document::open(Path::new("examples/first-success/design.fig")).unwrap();
        let expected = to_json(&doc.message().unwrap());
        let message = doc.take_message().unwrap();
        assert_eq!(to_json(&message), expected);
        assert!(!Tree::new(&message).nodes.is_empty());
        drop(message);
        assert_eq!(doc.message.capacity(), 0);
    }

    fn frame(version: u32, chunks: &[&[u8]]) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        out.extend(version.to_le_bytes());
        for chunk in chunks {
            out.extend((chunk.len() as u32).to_le_bytes());
            out.extend(*chunk);
        }
        out
    }

    #[test]
    fn compact_bytes_preserve_json_hashes_and_blob_contents() {
        for bytes in [vec![], vec![0, 127, 255]] {
            let compact = Value::Bytes(bytes.clone());
            let legacy = Value::Array(bytes.iter().copied().map(Value::Byte).collect());
            assert_eq!(to_json(&compact), to_json(&legacy));
            let object = |value| Value::Object("Blob", HashMap::from([("bytes", value)]));
            let blob = object(compact.clone());
            let tree = Tree {
                nodes: HashMap::new(),
                children: HashMap::new(),
                blobs: vec![&blob],
            };
            assert_eq!(tree.blob(0), Some(bytes));
            assert_eq!(tree.blob(1), None);
            let hash = |value| Value::Object("Image", HashMap::from([("hash", value)]));
            assert_eq!(to_json(&hash(compact)), to_json(&hash(legacy)));
        }
        assert_eq!(to_json(&Value::Bytes(vec![0, 255])), "<2 bytes>");
        let image = Value::Object(
            "Image",
            HashMap::from([("hash", Value::Bytes(vec![0, 255]))]),
        );
        assert_eq!(to_json(&image)["hash"], "00ff");
    }

    #[test]
    fn splits_version_and_chunks() {
        let data = frame(106, &[b"schema", b"message"]);
        let (version, chunks) = split_chunks(&data).unwrap();
        assert_eq!(
            (version, chunks),
            (106, vec![&b"schema"[..], &b"message"[..]])
        );
    }

    #[test]
    fn rejects_non_fig_data() {
        assert!(split_chunks(b"PK\x03\x04 not a canvas").is_err());
    }

    #[test]
    fn rejects_chunk_longer_than_data() {
        let mut data = frame(106, &[b"abc"]);
        data.truncate(data.len() - 1);
        assert!(split_chunks(&data).is_err());
    }

    #[test]
    fn unpacks_raw_deflate() {
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(b"kiwi").unwrap();
        assert_eq!(unpack(&encoder.finish().unwrap()).unwrap(), b"kiwi");
    }

    #[test]
    fn decompression_and_reads_obey_small_byte_budgets() {
        let bytes = b"small data";
        let mut deflate =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        deflate.write_all(bytes).unwrap();
        let deflate = deflate.finish().unwrap();
        let zstd = ruzstd::encoding::compress_to_vec(
            bytes.as_slice(),
            ruzstd::encoding::CompressionLevel::Uncompressed,
        );
        for compressed in [&deflate, &zstd] {
            for limit in [11, 10] {
                assert_eq!(
                    unpack_limited(compressed, limit, "test bytes").unwrap(),
                    bytes
                );
            }
            let error = unpack_limited(compressed, 9, "test bytes").unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("parse budget exceeded: test bytes (limit 9)")
            );
            assert_eq!(unpack_limited(compressed, 10, "test bytes").unwrap(), bytes);
        }
        assert!(read_limited(bytes.as_slice(), 9, "canvas bytes").is_err());
        assert_eq!(
            read_limited(bytes.as_slice(), 10, "canvas bytes").unwrap(),
            bytes
        );
    }

    #[test]
    fn image_reads_check_declared_actual_and_accumulated_bytes() {
        let mut remaining = 5;
        assert_eq!(read_image(&b"abc"[..], 3, &mut remaining).unwrap(), b"abc");
        assert_eq!(remaining, 2);
        assert!(read_image(std::io::empty(), 3, &mut remaining).is_err());
        assert!(read_image(&b"abc"[..], 0, &mut remaining).is_err());
        assert_eq!(
            remaining, 2,
            "failed reads do not spend the next call's budget"
        );
        assert_eq!(read_image(&b"ab"[..], 2, &mut remaining).unwrap(), b"ab");
        assert_eq!(remaining, 0);
        assert!(read_image(&b"x"[..], 0, &mut remaining).is_err());
        let mut remaining = parse::IMAGE_TOTAL_BYTES;
        let error =
            read_image(std::io::empty(), parse::IMAGE_BYTES + 1, &mut remaining).unwrap_err();
        assert_eq!(
            error.downcast_ref::<FigError>().unwrap().kind(),
            ErrorKind::ParseBudget
        );
        assert!(error.to_string().contains("image bytes"));
        assert_eq!(read_image(&b"ok"[..], 2, &mut remaining).unwrap(), b"ok");
    }

    #[test]
    fn chunk_count_has_a_finite_boundary() {
        let chunks = vec![b"".as_slice(); parse::CHUNKS];
        assert_eq!(
            split_chunks(&frame(106, &chunks)).unwrap().1.len(),
            parse::CHUNKS
        );
        let chunks = vec![b"".as_slice(); parse::CHUNKS + 1];
        assert!(
            split_chunks(&frame(106, &chunks))
                .unwrap_err()
                .to_string()
                .contains("chunk count")
        );
    }

    #[test]
    fn normalizes_url_node_ids() {
        assert_eq!(normalize_id("12-345"), "12:345");
    }

    #[test]
    fn takes_node_id_from_a_figma_url() {
        let url = "https://www.figma.com/design/AbC123xyz/Example?node-id=7-42&t=abc";
        assert_eq!(normalize_id(url), "7:42");
    }

    #[test]
    fn a_repeated_guid_keeps_the_last_records_parent_everywhere() {
        let g = |s, l| {
            Value::Object(
                "GUID",
                std::collections::HashMap::from([
                    ("sessionID", Value::UInt(s)),
                    ("localID", Value::UInt(l)),
                ]),
            )
        };
        let change = |id: (u32, u32), parent: (u32, u32)| {
            Value::Object(
                "NodeChange",
                std::collections::HashMap::from([
                    ("guid", g(id.0, id.1)),
                    (
                        "parentIndex",
                        Value::Object(
                            "ParentIndex",
                            std::collections::HashMap::from([
                                ("guid", g(parent.0, parent.1)),
                                ("position", Value::String("a".into())),
                            ]),
                        ),
                    ),
                ]),
            )
        };
        // 2:2 first sits under the document, then (last record) under 3:3, whose parent is 2:2.
        let message = Value::Object(
            "Message",
            std::collections::HashMap::from([(
                "nodeChanges",
                Value::Array(vec![
                    Value::Object(
                        "NodeChange",
                        std::collections::HashMap::from([("guid", g(0, 0))]),
                    ),
                    change((2, 2), (0, 0)),
                    change((3, 3), (2, 2)),
                    change((2, 2), (3, 3)),
                ]),
            )]),
        );
        let tree = Tree::new(&message);
        assert_eq!((tree.nodes.len(), tree.children.get("0:0")), (1, None));
    }

    #[test]
    fn nodes_in_a_parent_cycle_are_dropped() {
        let g = |s, l| {
            Value::Object(
                "GUID",
                std::collections::HashMap::from([
                    ("sessionID", Value::UInt(s)),
                    ("localID", Value::UInt(l)),
                ]),
            )
        };
        let change = |id: (u32, u32), parent: Option<(u32, u32)>| {
            let mut fields = std::collections::HashMap::from([("guid", g(id.0, id.1))]);
            if let Some((s, l)) = parent {
                fields.insert(
                    "parentIndex",
                    Value::Object(
                        "ParentIndex",
                        std::collections::HashMap::from([
                            ("guid", g(s, l)),
                            ("position", Value::String("a".into())),
                        ]),
                    ),
                );
            }
            Value::Object("NodeChange", fields)
        };
        // 2:2 and 3:3 are each other's parent; 4:4 hangs under the loop.
        let message = Value::Object(
            "Message",
            std::collections::HashMap::from([(
                "nodeChanges",
                Value::Array(vec![
                    change((0, 0), None),
                    change((1, 1), Some((0, 0))),
                    change((2, 2), Some((3, 3))),
                    change((3, 3), Some((2, 2))),
                    change((4, 4), Some((2, 2))),
                ]),
            )]),
        );
        let tree = Tree::new(&message);
        let mut ids: Vec<&str> = tree.nodes.keys().map(String::as_str).collect();
        ids.sort();
        assert_eq!(
            (ids, tree.children.len(), &tree.children["0:0"]),
            (vec!["0:0", "1:1"], 1, &vec!["1:1".to_owned()])
        );
    }

    #[test]
    fn guid_keys_require_unsigned_fields_and_reject_unset_ids() {
        let guid = |s, l| {
            Value::Object(
                "GUID",
                std::collections::HashMap::from([
                    ("sessionID", Value::UInt(s)),
                    ("localID", Value::UInt(l)),
                ]),
            )
        };
        assert_eq!(
            (guid_key(&guid(u32::MAX, u32::MAX)), guid_key(&guid(1, 2))),
            (None, Some("1:2".to_owned()))
        );
        for invalid in [
            Value::String("bad".into()),
            Value::Int(0),
            Value::Bool(false),
        ] {
            for field in ["sessionID", "localID"] {
                let mut value = guid(0, 0);
                if let Value::Object(_, fields) = &mut value {
                    fields.insert(field, invalid.clone());
                }
                assert!(guid_key(&value).is_none());
            }
        }
    }
}
