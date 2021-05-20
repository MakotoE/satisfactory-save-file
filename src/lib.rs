use crate::zlib_reader::ChunkedZLibReader;
use anyhow::{Error, Result};
use byteorder::{LittleEndian as L, ReadBytesExt};
use std::convert::TryInto;
use std::io::{Read, Seek};

pub mod zlib_reader;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SaveFile {
    pub save_header: i32,
    pub save_version: i32,
    pub build_version: i32,
    pub world_type: String,       // Make this an enum
    pub world_properties: String, // Make this a struct
    pub session_name: String,
    pub play_time: i32,         // Make this a duration type
    pub save_date: i64,         // Make this a date type
    pub session_visibility: u8, // Make this an enum
    pub editor_object_version: i32,
    pub mod_meta_data: String,
    pub is_modded_save: bool,
    pub save_objects: Vec<SaveObject>,
}

impl SaveFile {
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
            world_properties: read_string(file)?,
            session_name: read_string(file)?,
            play_time: file.read_i32::<L>()?,
            save_date: file.read_i64::<L>()?,
            session_visibility: file.read_u8()?,
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
        let file = File::open("test_files/new_world.sav").unwrap();
        let save_file = SaveFile::parse(&mut BufReader::new(file)).unwrap();
        dbg!(&save_file);

        assert_eq!(save_file.save_header, 8);
        assert_eq!(save_file.save_version, 25);
        assert_eq!(save_file.build_version, 152331);
        assert_eq!(save_file.world_type, "Persistent_Level");
        assert_eq!(save_file.session_name, "test_file");
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
