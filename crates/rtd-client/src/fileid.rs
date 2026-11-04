//! Bot-API file-id encoding and decoding.
//!
//! Implements the `persistent_id` version-4 format used by Telegram Desktop
//! (`type_id | dc | [file_reference] | [url] | id | access_hash | sub_version`,
//! outer RLE, base64url). Photo/thumbnail types additionally carry a
//! `photo_size_source`, which is not yet supported here.

use crate::error::{Error, Result};
use rtd_tg::{Document, DocumentAttribute, Photo, PhotoSize};

/// Flags packed into the file-id `type_id`.
const WEB_LOCATION_FLAG: u32 = 1 << 24;
/// Flag marking a file reference present.
const FILE_REFERENCE_FLAG: u32 = 1 << 25;
/// The latest inner sub-version.
const LATEST_SUB_VERSION: u8 = 34;
/// The outer persistent-id version.
const PERSISTENT_ID_VERSION: u8 = 4;

/// A Bot-API file type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    /// A thumbnail.
    Thumbnail,
    /// A profile photo.
    ProfilePhoto,
    /// A photo.
    Photo,
    /// A voice message.
    Voice,
    /// A video.
    Video,
    /// A document.
    Document,
    /// An encrypted file.
    Encrypted,
    /// A temporary file.
    Temp,
    /// A sticker.
    Sticker,
    /// An audio file.
    Audio,
    /// An animation.
    Animation,
    /// An encrypted thumbnail.
    EncryptedThumbnail,
    /// A wallpaper.
    Wallpaper,
    /// A video note.
    VideoNote,
    /// A secure raw file.
    SecureRaw,
    /// A secure file.
    Secure,
    /// A background.
    Background,
    /// A document served as a file.
    DocumentAsFile,
}

impl Type {
    /// Returns the numeric type id.
    #[must_use]
    pub fn id(self) -> u32 {
        self as u32
    }

    /// Returns the type for a numeric id, if valid.
    #[must_use]
    pub fn from_id(id: u32) -> Option<Self> {
        Some(match id {
            0 => Self::Thumbnail,
            1 => Self::ProfilePhoto,
            2 => Self::Photo,
            3 => Self::Voice,
            4 => Self::Video,
            5 => Self::Document,
            6 => Self::Encrypted,
            7 => Self::Temp,
            8 => Self::Sticker,
            9 => Self::Audio,
            10 => Self::Animation,
            11 => Self::EncryptedThumbnail,
            12 => Self::Wallpaper,
            13 => Self::VideoNote,
            14 => Self::SecureRaw,
            15 => Self::Secure,
            16 => Self::Background,
            17 => Self::DocumentAsFile,
            _ => return None,
        })
    }

    /// Reports whether this type carries a `photo_size_source`.
    #[must_use]
    pub fn has_photo_size(self) -> bool {
        matches!(self, Self::Thumbnail | Self::ProfilePhoto | Self::Photo)
    }
}

/// Photo metadata stored for photo/thumbnail file ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotoSizeSource {
    /// A legacy photo, keyed by its secret.
    Legacy {
        /// The photo secret.
        secret: i64,
    },
    /// A thumbnail of a file of a given type.
    Thumbnail {
        /// The type of the underlying file.
        file_type: Type,
        /// The thumbnail type marker.
        thumbnail_type: i32,
    },
    /// A small dialog photo.
    DialogPhotoSmall {
        /// The dialog peer id (with its TDLib kind bits).
        dialog_id: i64,
        /// The dialog access hash.
        dialog_access_hash: i64,
    },
    /// A big dialog photo.
    DialogPhotoBig {
        /// The dialog peer id.
        dialog_id: i64,
        /// The dialog access hash.
        dialog_access_hash: i64,
    },
    /// A sticker-set thumbnail.
    StickerSetThumbnail {
        /// The sticker-set id.
        set_id: i64,
        /// The sticker-set access hash.
        set_access_hash: i64,
    },
    /// A legacy full photo.
    FullLegacy {
        /// The volume id.
        volume_id: i64,
        /// The photo secret.
        secret: i64,
        /// The local id.
        local_id: i32,
    },
    /// A legacy small dialog photo.
    DialogPhotoSmallLegacy {
        /// The dialog peer id.
        dialog_id: i64,
        /// The dialog access hash.
        dialog_access_hash: i64,
        /// The volume id.
        volume_id: i64,
        /// The local id.
        local_id: i32,
    },
    /// A legacy big dialog photo.
    DialogPhotoBigLegacy {
        /// The dialog peer id.
        dialog_id: i64,
        /// The dialog access hash.
        dialog_access_hash: i64,
        /// The volume id.
        volume_id: i64,
        /// The local id.
        local_id: i32,
    },
    /// A legacy sticker-set thumbnail.
    StickerSetThumbnailLegacy {
        /// The sticker-set id.
        set_id: i64,
        /// The sticker-set access hash.
        set_access_hash: i64,
        /// The volume id.
        volume_id: i64,
        /// The local id.
        local_id: i32,
    },
    /// A versioned sticker-set thumbnail.
    StickerSetThumbnailVersion {
        /// The sticker-set id.
        set_id: i64,
        /// The sticker-set access hash.
        set_access_hash: i64,
        /// The sticker version.
        sticker_version: i32,
    },
}

impl PhotoSizeSource {
    /// Returns the wire type id.
    fn type_id(&self) -> i32 {
        match self {
            Self::Legacy { .. } => 0,
            Self::Thumbnail { .. } => 1,
            Self::DialogPhotoSmall { .. } => 2,
            Self::DialogPhotoBig { .. } => 3,
            Self::StickerSetThumbnail { .. } => 4,
            Self::FullLegacy { .. } => 5,
            Self::DialogPhotoSmallLegacy { .. } => 6,
            Self::DialogPhotoBigLegacy { .. } => 7,
            Self::StickerSetThumbnailLegacy { .. } => 8,
            Self::StickerSetThumbnailVersion { .. } => 9,
        }
    }

    /// Encodes the source (type id + variant fields).
    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.type_id().to_le_bytes());
        match self {
            Self::Legacy { secret } => out.extend_from_slice(&secret.to_le_bytes()),
            Self::Thumbnail {
                file_type,
                thumbnail_type,
            } => {
                out.extend_from_slice(&file_type.id().to_le_bytes());
                out.extend_from_slice(&thumbnail_type.to_le_bytes());
            }
            Self::DialogPhotoSmall {
                dialog_id,
                dialog_access_hash,
            }
            | Self::DialogPhotoBig {
                dialog_id,
                dialog_access_hash,
            } => {
                out.extend_from_slice(&dialog_id.to_le_bytes());
                out.extend_from_slice(&dialog_access_hash.to_le_bytes());
            }
            Self::StickerSetThumbnail {
                set_id,
                set_access_hash,
            } => {
                out.extend_from_slice(&set_id.to_le_bytes());
                out.extend_from_slice(&set_access_hash.to_le_bytes());
            }
            Self::FullLegacy {
                volume_id,
                secret,
                local_id,
            } => {
                out.extend_from_slice(&volume_id.to_le_bytes());
                out.extend_from_slice(&secret.to_le_bytes());
                out.extend_from_slice(&local_id.to_le_bytes());
            }
            Self::DialogPhotoSmallLegacy {
                dialog_id,
                dialog_access_hash,
                volume_id,
                local_id,
            }
            | Self::DialogPhotoBigLegacy {
                dialog_id,
                dialog_access_hash,
                volume_id,
                local_id,
            } => {
                out.extend_from_slice(&dialog_id.to_le_bytes());
                out.extend_from_slice(&dialog_access_hash.to_le_bytes());
                out.extend_from_slice(&volume_id.to_le_bytes());
                out.extend_from_slice(&local_id.to_le_bytes());
            }
            Self::StickerSetThumbnailLegacy {
                set_id,
                set_access_hash,
                volume_id,
                local_id,
            } => {
                out.extend_from_slice(&set_id.to_le_bytes());
                out.extend_from_slice(&set_access_hash.to_le_bytes());
                out.extend_from_slice(&volume_id.to_le_bytes());
                out.extend_from_slice(&local_id.to_le_bytes());
            }
            Self::StickerSetThumbnailVersion {
                set_id,
                set_access_hash,
                sticker_version,
            } => {
                out.extend_from_slice(&set_id.to_le_bytes());
                out.extend_from_slice(&set_access_hash.to_le_bytes());
                out.extend_from_slice(&sticker_version.to_le_bytes());
            }
        }
    }

    /// Decodes a source from `cursor`.
    fn decode(cursor: &mut Cursor<'_>) -> Result<Self> {
        let type_id = cursor.i32()?;
        let source = match type_id {
            0 => Self::Legacy {
                secret: cursor.i64()?,
            },
            1 => Self::Thumbnail {
                file_type: Type::from_id(cursor.u32()?)
                    .ok_or_else(|| Error::Decode("unknown thumbnail file type".to_owned()))?,
                thumbnail_type: cursor.i32()?,
            },
            2 => Self::DialogPhotoSmall {
                dialog_id: cursor.i64()?,
                dialog_access_hash: cursor.i64()?,
            },
            3 => Self::DialogPhotoBig {
                dialog_id: cursor.i64()?,
                dialog_access_hash: cursor.i64()?,
            },
            4 => Self::StickerSetThumbnail {
                set_id: cursor.i64()?,
                set_access_hash: cursor.i64()?,
            },
            5 => Self::FullLegacy {
                volume_id: cursor.i64()?,
                secret: cursor.i64()?,
                local_id: cursor.i32()?,
            },
            6 => Self::DialogPhotoSmallLegacy {
                dialog_id: cursor.i64()?,
                dialog_access_hash: cursor.i64()?,
                volume_id: cursor.i64()?,
                local_id: cursor.i32()?,
            },
            7 => Self::DialogPhotoBigLegacy {
                dialog_id: cursor.i64()?,
                dialog_access_hash: cursor.i64()?,
                volume_id: cursor.i64()?,
                local_id: cursor.i32()?,
            },
            8 => Self::StickerSetThumbnailLegacy {
                set_id: cursor.i64()?,
                set_access_hash: cursor.i64()?,
                volume_id: cursor.i64()?,
                local_id: cursor.i32()?,
            },
            9 => Self::StickerSetThumbnailVersion {
                set_id: cursor.i64()?,
                set_access_hash: cursor.i64()?,
                sticker_version: cursor.i32()?,
            },
            other => {
                return Err(Error::Decode(format!(
                    "unknown photo size source type {other}"
                )));
            }
        };
        Ok(source)
    }
}

/// A parsed file id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileId {
    /// The file type.
    pub type_: Type,
    /// The datacenter id.
    pub dc: i32,
    /// The file id.
    pub id: i64,
    /// The access hash.
    pub access_hash: i64,
    /// The file reference.
    pub file_reference: Vec<u8>,
    /// The URL, for a web location.
    pub url: String,
    /// The photo metadata, required for photo/thumbnail types.
    pub photo_size: Option<PhotoSizeSource>,
}

impl FileId {
    /// Encodes the file id into its base64url string.
    ///
    /// # Errors
    ///
    /// Returns an error for a photo/thumbnail type (photo-size sources are not
    /// supported yet).
    pub fn encode(&self) -> Result<String> {
        let has_web = !self.url.is_empty();
        let has_reference = !self.file_reference.is_empty();

        let mut body = Vec::new();
        let mut type_id = self.type_.id();
        if has_web {
            type_id |= WEB_LOCATION_FLAG;
        }
        if has_reference {
            type_id |= FILE_REFERENCE_FLAG;
        }
        body.extend_from_slice(&type_id.to_le_bytes());
        body.extend_from_slice(&(self.dc as u32).to_le_bytes());
        if has_reference {
            put_tl_bytes(&mut body, &self.file_reference);
        }
        if has_web {
            put_tl_string(&mut body, &self.url);
        } else {
            body.extend_from_slice(&self.id.to_le_bytes());
            body.extend_from_slice(&self.access_hash.to_le_bytes());
            if self.type_.has_photo_size() {
                let source = self.photo_size.as_ref().ok_or_else(|| {
                    Error::Unexpected(
                        "photo/thumbnail file ids require a photo_size source".to_owned(),
                    )
                })?;
                source.encode(&mut body);
            }
        }
        body.push(LATEST_SUB_VERSION);
        body.push(PERSISTENT_ID_VERSION);

        Ok(base64_url_encode(&rle_encode(&body)))
    }

    /// Decodes a file id from its base64url string.
    ///
    /// # Errors
    ///
    /// Returns an error when the string is not a supported file id.
    pub fn decode(encoded: &str) -> Result<Self> {
        let raw = base64_url_decode(encoded)
            .map_err(|reason| Error::Decode(format!("base64: {reason}")))?;
        let data = rle_decode(&raw);
        if data.len() < 2 {
            return Err(Error::Decode("RLE-decoded file id too small".to_owned()));
        }
        let version = data[data.len() - 1];
        if version != PERSISTENT_ID_VERSION {
            return Err(Error::Decode(format!("unknown file id version {version}")));
        }
        let body = &data[..data.len() - 1];

        let mut cursor = Cursor::new(body);
        let raw_type = cursor.u32()?;
        let has_web = raw_type & WEB_LOCATION_FLAG != 0;
        let has_reference = raw_type & FILE_REFERENCE_FLAG != 0;
        let type_id = raw_type & !(WEB_LOCATION_FLAG | FILE_REFERENCE_FLAG);
        let type_ = Type::from_id(type_id)
            .ok_or_else(|| Error::Decode(format!("unknown file type {type_id}")))?;

        let dc = cursor.u32()? as i32;
        let file_reference = if has_reference {
            cursor.tl_bytes()?
        } else {
            Vec::new()
        };
        let url = if has_web {
            cursor.tl_string()?
        } else {
            String::new()
        };
        let (id, access_hash) = if has_web {
            (0, 0)
        } else {
            (cursor.i64()?, cursor.i64()?)
        };
        let photo_size = if !has_web && type_.has_photo_size() {
            Some(PhotoSizeSource::decode(&mut cursor)?)
        } else {
            None
        };
        Ok(Self {
            type_,
            dc,
            id,
            access_hash,
            file_reference,
            url,
            photo_size,
        })
    }
}

/// A byte cursor over a file-id body.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .filter(|end| *end <= self.data.len())
            .ok_or_else(|| Error::Decode("truncated file id".to_owned()))?;
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn u32(&mut self) -> Result<u32> {
        let bytes: [u8; 4] = self.take(4)?.try_into().expect("four bytes");
        Ok(u32::from_le_bytes(bytes))
    }

    fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }

    fn i64(&mut self) -> Result<i64> {
        let bytes: [u8; 8] = self.take(8)?.try_into().expect("eight bytes");
        Ok(i64::from_le_bytes(bytes))
    }

    fn tl_bytes(&mut self) -> Result<Vec<u8>> {
        Ok(self.tl_slice()?.to_vec())
    }

    fn tl_string(&mut self) -> Result<String> {
        String::from_utf8(self.tl_slice()?.to_vec())
            .map_err(|_| Error::Decode("file id url is not UTF-8".to_owned()))
    }

    fn tl_slice(&mut self) -> Result<&'a [u8]> {
        let first = self.take(1)?[0];
        let len = if first < 254 {
            usize::from(first)
        } else {
            let extended = self.take(3)?;
            u32::from_le_bytes([extended[0], extended[1], extended[2], 0]) as usize
        };
        let value = self.take(len)?;
        // Padded to a 4-byte boundary.
        let total = 1 + (if first < 254 { 0 } else { 3 }) + len;
        let padding = (4 - total % 4) % 4;
        self.take(padding)?;
        Ok(value)
    }
}

/// Writes a length-prefixed, padded byte string.
fn put_tl_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    if bytes.len() < 254 {
        out.push(bytes.len() as u8);
        out.extend_from_slice(bytes);
        pad(out, 1 + bytes.len());
    } else {
        out.push(254);
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes()[..3]);
        out.extend_from_slice(bytes);
        pad(out, 4 + bytes.len());
    }
}

/// Writes a length-prefixed, padded string.
fn put_tl_string(out: &mut Vec<u8>, value: &str) {
    put_tl_bytes(out, value.as_bytes());
}

/// Pads `out` so that the `content`-length value aligns to 4 bytes.
fn pad(out: &mut Vec<u8>, content: usize) {
    let padding = (4 - content % 4) % 4;
    out.extend(std::iter::repeat_n(0, padding));
}

/// Run-length encodes zero runs (`0, count`).
fn rle_encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut count: u8 = 0;
    for &byte in data {
        if byte == 0 {
            count = count.saturating_add(1);
            continue;
        }
        if count > 0 {
            out.push(0);
            out.push(count);
            count = 0;
        }
        out.push(byte);
    }
    if count > 0 {
        out.push(0);
        out.push(count);
    }
    out
}

/// Reverses [`rle_encode`].
fn rle_decode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut last: Option<u8> = None;
    for &byte in data {
        match last {
            Some(0) => {
                out.extend(std::iter::repeat_n(0, usize::from(byte)));
                last = None;
            }
            Some(value) => {
                out.push(value);
                last = Some(byte);
            }
            None => last = Some(byte),
        }
    }
    if let Some(value) = last {
        out.push(value);
    }
    out
}

/// Encodes bytes as unpadded URL-safe base64.
fn base64_url_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map(u32::from);
        let b2 = chunk.get(2).copied().map(u32::from);
        let triple = (b0 << 16) | (b1.unwrap_or(0) << 8) | b2.unwrap_or(0);
        out.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        if b1.is_some() {
            out.push(ALPHABET[((triple >> 6) & 0x3f) as usize] as char);
        }
        if b2.is_some() {
            out.push(ALPHABET[(triple & 0x3f) as usize] as char);
        }
    }
    out
}

impl FileId {
    /// Returns the thumbnail size marker as a string, if this is a thumbnail id.
    fn thumb_size(&self) -> String {
        match &self.photo_size {
            Some(PhotoSizeSource::Thumbnail { thumbnail_type, .. }) => {
                char::from_u32(u32::try_from(*thumbnail_type).unwrap_or(0))
                    .map(|c| c.to_string())
                    .unwrap_or_default()
            }
            _ => String::new(),
        }
    }

    /// Returns the `InputFileLocation` for this file id.
    ///
    /// Returns `None` for a web location (use
    /// [`FileId::as_input_web_file_location`] instead) and for types that have
    /// no input-file location.
    #[must_use]
    pub fn as_input_file_location(&self) -> Option<rtd_tg::InputFileLocation> {
        use rtd_tg::InputFileLocation;
        if !self.url.is_empty() {
            return None;
        }
        Some(match self.type_ {
            Type::Encrypted | Type::EncryptedThumbnail => {
                InputFileLocation::InputEncryptedFileLocation {
                    id: self.id,
                    access_hash: self.access_hash,
                }
            }
            Type::Document
            | Type::DocumentAsFile
            | Type::Video
            | Type::VideoNote
            | Type::Audio
            | Type::Voice
            | Type::Animation
            | Type::Sticker => InputFileLocation::InputDocumentFileLocation {
                id: self.id,
                access_hash: self.access_hash,
                file_reference: self.file_reference.clone(),
                thumb_size: self.thumb_size(),
            },
            Type::Photo
            | Type::ProfilePhoto
            | Type::Thumbnail
            | Type::Wallpaper
            | Type::Background => InputFileLocation::InputPhotoFileLocation {
                id: self.id,
                access_hash: self.access_hash,
                file_reference: self.file_reference.clone(),
                thumb_size: self.thumb_size(),
            },
            _ => return None,
        })
    }

    /// Returns the `InputWebFileLocation` for a web file id.
    #[must_use]
    pub fn as_input_web_file_location(&self) -> Option<rtd_tg::InputWebFileLocation> {
        if self.url.is_empty() {
            return None;
        }
        Some(rtd_tg::InputWebFileLocation::InputWebFileLocation {
            url: self.url.clone(),
            access_hash: self.access_hash,
        })
    }
}

/// Builds a document file id from a `Document`.
///
/// The file type is derived from the document's attributes (sticker, animation,
/// voice/audio, video or video note), falling back to a plain document.
#[must_use]
pub fn from_document(document: &Document, dc: i32) -> Option<FileId> {
    let Document::Document {
        id,
        access_hash,
        file_reference,
        attributes,
        ..
    } = document
    else {
        return None;
    };
    Some(FileId {
        type_: document_type(attributes),
        dc,
        id: *id,
        access_hash: *access_hash,
        file_reference: file_reference.clone(),
        url: String::new(),
        photo_size: None,
    })
}

/// Builds a photo file id from a `Photo`, choosing the largest usable size.
///
/// The chosen size's type marker becomes the file id's thumbnail type, and the
/// file type is stored as [`Type::Photo`].
#[must_use]
pub fn from_photo(photo: &Photo, dc: i32) -> Option<FileId> {
    let Photo::Photo {
        id,
        access_hash,
        file_reference,
        sizes,
        ..
    } = photo
    else {
        return None;
    };
    let thumbnail_type = sizes
        .iter()
        .filter_map(photo_size_info)
        .max_by_key(|(_, w, h)| i64::from(*w) * i64::from(*h))
        .map(|(thumbnail_type, _, _)| thumbnail_type)?;
    Some(FileId {
        type_: Type::Photo,
        dc,
        id: *id,
        access_hash: *access_hash,
        file_reference: file_reference.clone(),
        url: String::new(),
        photo_size: Some(PhotoSizeSource::Thumbnail {
            file_type: Type::Photo,
            thumbnail_type,
        }),
    })
}

/// Extracts `(type marker, width, height)` from a dimensionless photo size.
fn photo_size_info(size: &PhotoSize) -> Option<(i32, i32, i32)> {
    match size {
        PhotoSize::PhotoSize { type_, w, h, .. }
        | PhotoSize::PhotoSizeProgressive { type_, w, h, .. }
        | PhotoSize::PhotoCachedSize { type_, w, h, .. } => {
            Some((i32::from(type_.bytes().next()?), *w, *h))
        }
        _ => None,
    }
}

/// Maps document attributes to a Bot-API file type.
fn document_type(attributes: &[DocumentAttribute]) -> Type {
    let mut kind = Type::Document;
    for attribute in attributes {
        match attribute {
            DocumentAttribute::DocumentAttributeSticker { .. }
            | DocumentAttribute::DocumentAttributeCustomEmoji { .. } => return Type::Sticker,
            DocumentAttribute::DocumentAttributeAnimated => kind = Type::Animation,
            DocumentAttribute::DocumentAttributeAudio { voice, .. } => {
                kind = if *voice { Type::Voice } else { Type::Audio };
            }
            DocumentAttribute::DocumentAttributeVideo { round_message, .. } => {
                kind = if *round_message {
                    Type::VideoNote
                } else {
                    Type::Video
                };
            }
            _ => {}
        }
    }
    kind
}

/// Decodes unpadded URL-safe base64.
fn base64_url_decode(input: &str) -> core::result::Result<Vec<u8>, &'static str> {
    let mut out = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for byte in input.bytes() {
        if byte == b'=' {
            continue;
        }
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err("invalid base64url character"),
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{FileId, Type};

    #[test]
    fn document_round_trips() {
        let id = FileId {
            type_: Type::Document,
            dc: 2,
            id: 0x1122_3344_5566_7788,
            access_hash: -9,
            file_reference: vec![1, 2, 0, 0, 3],
            url: String::new(),
            photo_size: None,
        };
        let encoded = id.encode().unwrap();
        assert_eq!(FileId::decode(&encoded).unwrap(), id);
    }

    #[test]
    fn web_location_round_trips() {
        let id = FileId {
            type_: Type::Document,
            dc: 4,
            id: 0,
            access_hash: 0,
            file_reference: Vec::new(),
            url: "https://example.com/x".to_owned(),
            photo_size: None,
        };
        let encoded = id.encode().unwrap();
        assert_eq!(FileId::decode(&encoded).unwrap(), id);
    }

    #[test]
    fn photo_thumbnail_round_trips_with_source() {
        use super::PhotoSizeSource;
        let id = FileId {
            type_: Type::Thumbnail,
            dc: 2,
            id: 1,
            access_hash: 2,
            file_reference: Vec::new(),
            url: String::new(),
            photo_size: Some(PhotoSizeSource::Thumbnail {
                file_type: Type::Photo,
                thumbnail_type: b'x' as i32,
            }),
        };
        let encoded = id.encode().unwrap();
        assert_eq!(FileId::decode(&encoded).unwrap(), id);
    }

    #[test]
    fn rle_round_trips_zero_runs() {
        let data = vec![1, 0, 0, 0, 2, 0, 3];
        assert_eq!(super::rle_decode(&super::rle_encode(&data)), data);
    }

    #[test]
    fn builds_a_document_file_id() {
        use rtd_tg::{Document, DocumentAttribute};

        let document = Document::Document {
            flags: 0,
            id: 42,
            access_hash: 7,
            file_reference: vec![1, 2, 3],
            date: 0,
            mime_type: "video/mp4".to_owned(),
            size: 10,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes: vec![DocumentAttribute::DocumentAttributeVideo {
                flags: 0,
                round_message: false,
                supports_streaming: false,
                nosound: false,
                duration: 1.0,
                w: 1,
                h: 1,
                preload_prefix_size: None,
                video_start_ts: None,
                video_codec: None,
            }],
        };
        let id = super::from_document(&document, 4).expect("file id");
        assert_eq!(id.type_, Type::Video);
        assert_eq!(id.dc, 4);
        assert_eq!(id.id, 42);
        // It round-trips through its encoded form.
        let decoded = FileId::decode(&id.encode().expect("encode")).expect("decode");
        assert_eq!(decoded, id);

        // An empty document has no usable file id.
        assert!(super::from_document(&Document::DocumentEmpty { id: 0 }, 2).is_none());
    }

    #[test]
    fn builds_a_photo_file_id() {
        use rtd_tg::{Photo, PhotoSize};

        let photo = Photo::Photo {
            flags: 0,
            has_stickers: false,
            id: 9,
            access_hash: 11,
            file_reference: vec![4, 5],
            date: 0,
            sizes: vec![
                PhotoSize::PhotoSize {
                    type_: "m".to_owned(),
                    w: 100,
                    h: 100,
                    size: 10,
                },
                PhotoSize::PhotoSize {
                    type_: "y".to_owned(),
                    w: 800,
                    h: 600,
                    size: 20,
                },
            ],
            video_sizes: None,
            dc_id: 2,
        };
        let id = super::from_photo(&photo, 3).expect("file id");
        assert_eq!(id.type_, Type::Photo);
        assert_eq!(
            id.photo_size,
            Some(super::PhotoSizeSource::Thumbnail {
                file_type: Type::Photo,
                thumbnail_type: i32::from(b'y'),
            })
        );
        // It round-trips through its encoded form.
        let decoded = FileId::decode(&id.encode().expect("encode")).expect("decode");
        assert_eq!(decoded, id);
    }

    #[test]
    fn maps_to_input_locations() {
        use rtd_tg::{InputFileLocation, InputWebFileLocation};

        let document = FileId {
            type_: Type::Document,
            dc: 2,
            id: 1,
            access_hash: 2,
            file_reference: vec![9],
            url: String::new(),
            photo_size: None,
        };
        match document.as_input_file_location() {
            Some(InputFileLocation::InputDocumentFileLocation {
                id,
                access_hash,
                thumb_size,
                ..
            }) => {
                assert_eq!((id, access_hash), (1, 2));
                assert_eq!(thumb_size, "");
            }
            other => panic!("expected document location, got {other:?}"),
        }
        assert!(document.as_input_web_file_location().is_none());

        let web = FileId {
            type_: Type::Document,
            dc: 2,
            id: 0,
            access_hash: 5,
            file_reference: Vec::new(),
            url: "https://example.com/x".to_owned(),
            photo_size: None,
        };
        assert!(web.as_input_file_location().is_none());
        match web.as_input_web_file_location() {
            Some(InputWebFileLocation::InputWebFileLocation { url, access_hash }) => {
                assert_eq!(url, "https://example.com/x");
                assert_eq!(access_hash, 5);
            }
            other => panic!("expected web location, got {other:?}"),
        }
    }
}
