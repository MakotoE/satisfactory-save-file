use crate::SaveObject::SaveComponent;
use anyhow::{Error, Result};
use byteorder::{LittleEndian as L, ReadBytesExt};
use flate2::read::ZlibDecoder;
use std::io::{Read, Seek};

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
    pub fn new<R>(file: &mut R) -> Result<SaveFile>
    where
        R: Read + Seek,
    {
        let mut buffers: (Vec<u8>, Vec<u16>) = (Vec::new(), Vec::new());
        let mut save_file = SaveFile::default();
        save_file.parse(file, &mut buffers)?;
        Ok(save_file)
    }

    pub fn parse<R>(&mut self, file: &mut R, buffers: &mut (Vec<u8>, Vec<u16>)) -> Result<()>
    where
        R: Read + Seek,
    {
        // https://github.com/Goz3rr/SatisfactorySaveEditor
        // https://satisfactory.fandom.com/wiki/Save_files (outdated info)

        self.save_header = file.read_i32::<L>()?;
        self.save_version = file.read_i32::<L>()?;
        self.build_version = file.read_i32::<L>()?;
        read_string(file, buffers, &mut self.world_type)?;
        read_string(file, buffers, &mut self.world_properties)?;
        read_string(file, buffers, &mut self.session_name)?;
        self.play_time = file.read_i32::<L>()?;
        self.save_date = file.read_i64::<L>()?;
        self.session_visibility = file.read_u8()?;
        self.editor_object_version = file.read_i32::<L>()?;
        read_string(file, buffers, &mut self.mod_meta_data)?;
        self.is_modded_save = file.read_i32::<L>()? > 0;

        if file.read_i64::<L>()? != 0x9E2A83C1 {
            log::error!("unexpected package file tag");
        }
        if file.read_i64::<L>()? != 131072 {
            log::error!("unexpected max chunk size");
        }
        let chunk_compressed_length = file.read_i64::<L>()?;
        let chunk_uncompressed_length = file.read_i64::<L>()?;
        let chunk_compressed_length_1 = file.read_i64::<L>()?;
        let chunk_uncompressed_length_1 = file.read_i64::<L>()?;

        let mut decoder = ZlibDecoder::new(file);

        let data_length = decoder.read_i32::<L>()?;

        let world_object_count = decoder.read_u32::<L>()?;
        for _ in 0..world_object_count {
            self.save_objects
                .push(SaveObject::parse(&mut decoder, buffers)?);
        }
        Ok(())
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
    pub fn parse<R>(file: &mut R, buffers: &mut (Vec<u8>, Vec<u16>)) -> Result<Self>
    where
        R: Read,
    {
        let object_type = file.read_i32::<L>()?;
        let mut type_path = String::new();
        read_string(file, buffers, &mut type_path)?;
        let mut root_object = String::new();
        read_string(file, buffers, &mut root_object)?;
        let mut instance_name = String::new();
        read_string(file, buffers, &mut instance_name)?;
        Ok(match object_type {
            0 => {
                let mut parent_entity_name = String::new();
                read_string(file, buffers, &mut parent_entity_name)?;
                SaveObject::SaveComponent {
                    type_path,
                    root_object,
                    instance_name,
                    parent_entity_name,
                }
            }
            1 => SaveObject::SaveEntity {
                type_path,
                root_object,
                instance_name,
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

fn read_string<R>(file: &mut R, buffers: &mut (Vec<u8>, Vec<u16>), s: &mut String) -> Result<()>
where
    R: Read,
{
    s.clear();

    let length = file.read_i32::<L>()?;

    if length < 0 {
        buffers.1.clear();
        buffers
            .1
            .resize(((-length) as usize).saturating_sub(1) / 2, 0);
        file.read_u16_into::<L>(&mut buffers.1)?;
        s.clone_from(&String::from_utf16_lossy(&buffers.1));
    } else {
        buffers.0.clear();
        buffers
            .0
            .resize((length.abs() as usize).saturating_sub(1), b'\0');
        file.read_exact(&mut buffers.0)?;
        if length > 0 {
            // Skip null char
            file.read_u8()?;
        }
        s.push_str(std::str::from_utf8(&buffers.0)?);
    };

    Ok(())
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
    use crate::{read_string, SaveFile};
    use std::fs::File;
    use std::io::Cursor;
    use std::iter::once;

    #[test]
    fn parse() {
        env_logger::builder().is_test(true).try_init().unwrap();
        let mut file = File::open("test_files/new_world.sav").unwrap();
        let save_file = SaveFile::new(&mut file).unwrap();
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
        let mut buffers: (Vec<u8>, Vec<u16>) = (Vec::new(), Vec::new());
        let mut result = String::new();
        {
            // Empty file
            let mut data = Cursor::new(Vec::new());
            let result = read_string(&mut data, &mut buffers, &mut result);
            assert!(result.is_err());
        }
        {
            // Just the prefix
            let mut data = &0_i32.to_le_bytes()[..];
            read_string(&mut data, &mut buffers, &mut result).unwrap();
            assert_eq!(result, "");
        }
        // Various strings
        for test_string in &["", "a", "abc"] {
            let encoded = to_encoding(test_string.as_bytes());
            read_string(&mut encoded.as_slice(), &mut buffers, &mut result).unwrap();
            assert_eq!(result, *test_string);
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
            read_string(&mut encoded.as_slice(), &mut buffers, &mut result).unwrap();
            assert_eq!(result, test_string);
        }
    }
}
