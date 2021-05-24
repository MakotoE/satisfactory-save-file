//! `SaveFile` represents save files in Satisfactory. Use `SaveFile::parse()` to read save files.

use crate::zlib_reader::ChunkedZLibReader;
use crate::SessionVisiblity::{SvFriendsOnly, SvInvalid, SvPrivate};
use anyhow::{Error, Result};
use byteorder::{LittleEndian as L, ReadBytesExt};
use chrono::{DateTime, Duration, TimeZone, Utc};
use std::collections::HashMap;
use std::convert::TryInto;
use std::io::{Read, Seek};

pub mod zlib_reader;

/// Satisfactory save file.
#[derive(Debug, Clone, PartialEq)]
pub struct SaveFile {
    pub save_header: i32,
    pub save_version: i32,
    pub build_version: i32,
    pub world_type: String,
    pub world_properties: WorldProperties,
    pub session_name: String,
    pub play_time: Duration,
    pub save_date: DateTime<Utc>,
    pub session_visibility: SessionVisiblity,
    pub editor_object_version: i32,
    pub mod_meta_data: String,
    pub is_modded_save: bool,
    pub save_objects: Vec<SaveObject>,
}

impl SaveFile {
    /// Reads satisfactory save file to SaveFile struct.
    ///
    /// Save files are stored in `%localappdata%\FactoryGame\Saved\SaveGames\<your id>` and it has a
    /// `.sav` extension.
    ///
    /// Tested with build version 152331.
    ///
    /// Do not pass a BufReader. I don't know why this fails with BufReader.
    pub fn parse<R>(file: &mut R) -> Result<SaveFile>
    where
        R: Read + Seek,
    {
        // https://github.com/Goz3rr/SatisfactorySaveEditor
        // https://satisfactory.fandom.com/wiki/Save_files (outdated info)

        let mut save_file = SaveFile {
            save_header: file.read_i32::<L>()?,
            save_version: file.read_i32::<L>()?,
            build_version: file.read_i32::<L>()?,
            world_type: read_string(file)?,
            world_properties: WorldProperties::new(&read_string(file)?)?,
            session_name: read_string(file)?,
            play_time: Duration::seconds(file.read_i32::<L>()?.try_into()?),
            save_date: SaveFile::convert_date(file.read_i64::<L>()?),
            session_visibility: SessionVisiblity::from_u8(file.read_u8()?)?,
            editor_object_version: file.read_i32::<L>()?,
            mod_meta_data: read_string(file)?,
            is_modded_save: file.read_i32::<L>()? > 0,
            save_objects: Vec::new(),
        };

        let mut decoder = ChunkedZLibReader::new(file)?;
        let world_object_count = decoder.read_u32::<L>()?;
        save_file.save_objects.reserve(world_object_count as usize);
        for _ in 0..world_object_count {
            save_file
                .save_objects
                .push(SaveObject::parse(&mut decoder)?);
        }
        Ok(save_file)
    }

    fn zero_date() -> DateTime<Utc> {
        chrono::Utc.ymd(1, 1, 1).and_hms(12, 0, 0)
    }

    fn convert_date(n: i64) -> DateTime<Utc> {
        SaveFile::zero_date() + Duration::nanoseconds(n) * 100
    }
}

impl Default for SaveFile {
    fn default() -> Self {
        Self {
            save_header: Default::default(),
            save_version: Default::default(),
            build_version: Default::default(),
            world_type: Default::default(),
            world_properties: Default::default(),
            session_name: Default::default(),
            play_time: Duration::zero(),
            save_date: SaveFile::zero_date(),
            session_visibility: Default::default(),
            editor_object_version: Default::default(),
            mod_meta_data: Default::default(),
            is_modded_save: Default::default(),
            save_objects: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct WorldProperties {
    pub start_loc: String,
    pub session_name: String,
    pub visibility: SessionVisiblity,
}

impl WorldProperties {
    pub fn new(s: &str) -> Result<WorldProperties> {
        let mut map: HashMap<&str, &str> = s
            .split('?')
            .skip(1) // Nothing before first "?"
            .map(|s| {
                s.split_once("=")
                    .ok_or_else(|| Error::msg(format!("invalid property: {}", s)))
            })
            .collect::<Result<HashMap<&str, &str>>>()?;

        let not_found_error = || Error::msg("property not found");
        Ok(WorldProperties {
            start_loc: map
                .remove("startloc")
                .ok_or_else(not_found_error)?
                .to_string(),
            session_name: map
                .remove("sessionName")
                .ok_or_else(not_found_error)?
                .to_string(),
            visibility: SessionVisiblity::parse(
                map.remove("Visibility").ok_or_else(not_found_error)?,
            )?,
        })
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SessionVisiblity {
    SvPrivate,
    SvFriendsOnly,
    SvInvalid,
}

impl SessionVisiblity {
    pub fn from_u8(n: u8) -> Result<SessionVisiblity> {
        Ok(match n {
            0 => SvPrivate,
            1 => SvFriendsOnly,
            2 => SvInvalid,
            _ => return Err(Error::msg(format!("invalid n: {}", n))),
        })
    }

    pub fn parse(s: &str) -> Result<SessionVisiblity> {
        Ok(match s {
            "SV_Private" => SvPrivate,
            "SV_FriendsOnly" => SvFriendsOnly,
            "SV_Invalid" => SvInvalid,
            _ => return Err(Error::msg(format!("invalid s: {}", s))),
        })
    }
}

impl Default for SessionVisiblity {
    fn default() -> Self {
        SvPrivate
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SaveObject {
    SaveComponent {
        type_path: String,
        root_object: String,
        instance_name: String,
        parent_entity_name: String,
    },
    SaveEntity {
        type_path: String,
        root_object: String,
        instance_name: String,
        need_transform: bool,
        rotation: Vector4,
        position: Vector3,
        scale: Vector3,
        was_placed_in_level: bool,
    },
}

impl SaveObject {
    pub fn parse<R>(file: &mut R) -> Result<Self>
    where
        R: Read,
    {
        let object_type = file.read_i32::<L>()?;
        Ok(match object_type {
            0 => SaveObject::SaveComponent {
                type_path: read_string(file)?,
                root_object: read_string(file)?,
                instance_name: read_string(file)?,
                parent_entity_name: read_string(file)?,
            },
            1 => SaveObject::SaveEntity {
                type_path: read_string(file)?,
                root_object: read_string(file)?,
                instance_name: read_string(file)?,
                need_transform: file.read_i32::<L>()? == 1,
                rotation: Vector4::parse(file)?,
                position: Vector3::parse(file)?,
                scale: Vector3::parse(file)?,
                was_placed_in_level: file.read_i32::<L>()? == 1,
            },
            n => return Err(Error::msg(format!("unknown object type: {}", n))),
        })
    }
}

fn read_string<R>(file: &mut R) -> Result<String>
where
    R: Read,
{
    let length = file.read_i32::<L>()?;

    Ok(if length < 0 {
        let mut buffer: Vec<u16> = Vec::new();
        buffer.resize(((-length) as usize).saturating_sub(1) / 2, 0);
        file.read_u16_into::<L>(&mut buffer)?;
        String::from_utf16_lossy(&buffer)
    } else {
        let mut buffer: Vec<u8> = Vec::new();
        buffer.resize((length.abs() as usize).saturating_sub(1), b'\0');
        file.read_exact(&mut buffer)?;
        if length > 0 {
            // Skip null char
            let b = file.read_u8()?;
            debug_assert_eq!(b, b'\0');
        }
        String::from_utf8_lossy(&buffer).into_owned()
    })
}

#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

impl Vector2 {
    pub fn parse<R>(file: &mut R) -> Result<Self>
    where
        R: Read,
    {
        Ok(Self {
            x: file.read_f32::<L>()?,
            y: file.read_f32::<L>()?,
        })
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    pub fn parse<R>(file: &mut R) -> Result<Self>
    where
        R: Read,
    {
        Ok(Self {
            x: file.read_f32::<L>()?,
            y: file.read_f32::<L>()?,
            z: file.read_f32::<L>()?,
        })
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vector4 {
    pub fn parse<R>(file: &mut R) -> Result<Self>
    where
        R: Read,
    {
        Ok(Self {
            x: file.read_f32::<L>()?,
            y: file.read_f32::<L>()?,
            z: file.read_f32::<L>()?,
            w: file.read_f32::<L>()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::{BufReader, Cursor};
    use std::iter::once;

    #[test]
    fn parse() {
        env_logger::builder().is_test(true).try_init().unwrap();
        let mut file = File::open("test_files/new_world.sav").unwrap();
        let save_file = SaveFile::parse(&mut file).unwrap();
        assert_eq!(save_file.save_header, 8);
        assert_eq!(save_file.save_version, 25);
        assert_eq!(save_file.build_version, 152331);
        assert_eq!(save_file.world_type, "Persistent_Level");
        assert_eq!(save_file.session_name, "test_file");
        assert_eq!(save_file.save_objects.len(), 13920);
        assert!(matches!(
            &save_file.save_objects[0],
            SaveObject::SaveEntity { type_path, .. }
                if type_path == "/Script/FactoryGame.FGFoliageRemoval"
        ));

        // Demonstrates how it fails when reading from BufReader
        let mut file = File::open("test_files/new_world.sav").unwrap();
        assert!(SaveFile::parse(&mut BufReader::new(file)).is_err());
    }

    #[test]
    fn world_properties() {
        assert!(WorldProperties::new("").is_err());
        let string = "?startloc=Grass Fields?sessionName=test_file?Visibility=SV_Private";
        let result = WorldProperties::new(string).unwrap();
        assert_eq!(result.start_loc, "Grass Fields");
        assert_eq!(result.session_name, "test_file");
        assert_eq!(result.visibility, SessionVisiblity::SvPrivate);
    }

    fn to_encoding(b: &[u8]) -> Vec<u8> {
        (b.len() as i32 + 1) // length prefix
            .to_le_bytes()
            .iter()
            .chain(b.iter()) // data
            .chain(once(&b'\0')) // null terminator
            .copied()
            .collect()
    }

    #[test]
    fn test_read_string() {
        {
            // Empty file
            let mut data = Cursor::new(Vec::new());
            assert!(read_string(&mut data).is_err());
        }
        {
            // Just the prefix
            let mut data = &0_i32.to_le_bytes()[..];
            assert_eq!(read_string(&mut data).unwrap(), "");
        }
        // Various strings
        for test_string in &["", "a", "abc"] {
            let encoded = to_encoding(test_string.as_bytes());
            assert_eq!(read_string(&mut encoded.as_slice()).unwrap(), *test_string);
        }
        {
            // UTF-16
            let test_string = "abc";
            let utf16: Vec<u16> = test_string.encode_utf16().collect();
            let mut utf16_bytes: Vec<u8> = Vec::new();
            for n in utf16 {
                utf16_bytes.extend_from_slice(&n.to_le_bytes());
            }
            let encoded: Vec<u8> = (-(utf16_bytes.len() as i32 + 2))
                .to_le_bytes()
                .iter()
                .chain(utf16_bytes.iter())
                .chain([b'\0', b'\0'].iter())
                .copied()
                .collect();
            assert_eq!(read_string(&mut encoded.as_slice()).unwrap(), test_string);
        }
    }
}
