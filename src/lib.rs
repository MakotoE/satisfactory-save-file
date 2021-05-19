use anyhow::{Error, Result};
use byteorder::{LittleEndian as L, ReadBytesExt};
use flate2::read::ZlibDecoder;
use std::convert::TryInto;
use std::io::{Read, Seek, SeekFrom};

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

        let mut sf = SaveFile::default();
        sf.save_header = file.read_i32::<L>()?;
        sf.save_version = file.read_i32::<L>()?;
        sf.build_version = file.read_i32::<L>()?;
        sf.world_type = read_string(file)?;
        sf.world_properties = read_string(file)?;
        sf.session_name = read_string(file)?;
        sf.play_time = file.read_i32::<L>()?;
        sf.save_date = file.read_i64::<L>()?;
        sf.session_visibility = file.read_u8()?;
        sf.editor_object_version = file.read_i32::<L>()?;
        sf.mod_meta_data = read_string(file)?;
        sf.is_modded_save = file.read_i32::<L>()? > 0;

        let mut decoder = ChunkedZLibReader::new(file)?;

        let world_object_count = decoder.read_u32::<L>()?;
        for _ in 0..world_object_count {
            sf.save_objects.push(SaveObject::parse(&mut decoder)?);
        }
        Ok(sf)
    }
}

#[derive(Debug)]
pub struct ChunkedZLibReader<R>
where
    R: Read + Seek,
{
    decoder: Option<ZlibDecoder<R>>,
    chunk_end: u64,
}

impl<R: Read + Seek> ChunkedZLibReader<R> {
    pub fn new(mut file: R) -> Result<Self> {
        let chunk_length = ChunkedZLibReader::read_header(&mut file)?;
        let current_position = file.seek(SeekFrom::Current(0))?;
        let mut decoder = ZlibDecoder::new(file);

        // Data length
        decoder.read_i32::<L>()?;

        Ok(Self {
            decoder: Some(decoder),
            chunk_end: current_position + chunk_length,
        })
    }

    fn read_header(file: &mut R) -> Result<u64> {
        let package_file_tag = file.read_i64::<L>()?;
        if package_file_tag != 0x9E2A83C1 {
            log::error!("unexpected package file tag: {}", package_file_tag);
        }
        let max_chunk_size = file.read_i64::<L>()?;
        if max_chunk_size != 131072 {
            log::error!("unexpected max chunk size {}", max_chunk_size);
        }

        let chunk_compressed_length = file.read_i64::<L>()?;
        // Uncompressed length
        file.read_i64::<L>()?;

        // Duplicate of compressed and uncompressed lengths
        file.read_i64::<L>()?;
        file.read_i64::<L>()?;

        Ok(chunk_compressed_length.try_into()?)
    }
}

impl<R: Read + Seek> Read for ChunkedZLibReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let result = self.decoder.as_mut().unwrap().read(buf);

        if let Ok(bytes_read) = result {
            // End of chunk
            if bytes_read < buf.len() {
                let mut file = self.decoder.take().unwrap().into_inner();

                file.seek(SeekFrom::Start(self.chunk_end))?;

                let chunk_length = match ChunkedZLibReader::read_header(&mut file) {
                    Ok(n) => n,
                    Err(e) => return Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
                };

                const CHUNK_HEADER_LENGTH: u64 = 24;
                self.chunk_end = self.chunk_end + CHUNK_HEADER_LENGTH + chunk_length;

                self.decoder = Some(ZlibDecoder::new(file));

                // Data length
                self.decoder.as_mut().unwrap().read_i32::<L>()?;
            }
        }

        result
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
            file.read_u8()?;
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
    use std::io::Cursor;
    use std::iter::once;

    #[test]
    fn parse() {
        env_logger::builder().is_test(true).try_init().unwrap();
        let mut file = File::open("test_files/new_world.sav").unwrap();
        let save_file = SaveFile::parse(&mut file).unwrap();
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
