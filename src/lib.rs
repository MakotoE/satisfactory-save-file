use anyhow::Result;
use byteorder::{LittleEndian as L, ReadBytesExt};
use std::io::Read;

#[derive(Debug, Default, Clone)]
pub struct SaveFile {
    pub save_header: i32,
    pub save_version: i32,
    pub build_version: i32,
    pub world_type: String,       // Make this an enum
    pub world_properties: String, // Make this a struct
    pub play_time: i32,           // Make this a duration type
    pub save_date: i64,           // Make this a date type
    pub session_visibility: u8,   // Make this an enum
}

impl SaveFile {
    pub fn new<R>(file: &mut R) -> Result<SaveFile>
    where
        R: Read,
    {
        let mut save_file = SaveFile::default();
        save_file.parse(file)?;
        Ok(save_file)
    }

    pub fn parse<R>(&mut self, file: &mut R) -> Result<()>
    where
        R: Read,
    {
        // https://satisfactory.fandom.com/wiki/Save_files

        let mut buffers: (Vec<u8>, Vec<u16>) = (Vec::new(), Vec::new());

        self.save_header = file.read_i32::<L>()?;
        self.save_version = file.read_i32::<L>()?;
        self.build_version = file.read_i32::<L>()?;
        read_string(file, &mut buffers, &mut self.world_type)?;
        read_string(file, &mut buffers, &mut self.world_properties)?;
        self.play_time = file.read_i32::<L>()?;
        self.save_date = file.read_i64::<L>()?;
        self.session_visibility = file.read_u8()?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct WorldObject {}

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
        // Skip null char
        file.read_u8()?;
        s.push_str(std::str::from_utf8(&buffers.0)?);
    };

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{read_string, SaveFile};
    use std::fs::File;
    use std::iter::once;

    #[test]
    fn parse() {
        let mut file = File::open("test_files/new_world.sav").unwrap();
        let mut save_file = SaveFile::default();
        save_file.parse(&mut file).unwrap();
        dbg!(&save_file);
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
            let result = read_string(&mut "".as_bytes(), &mut buffers, &mut result);
            assert!(result.is_err());
        }
        {
            let test_string = "";
            let encoded = to_encoding(test_string.as_bytes());
            read_string(&mut encoded.as_slice(), &mut buffers, &mut result).unwrap();
            assert_eq!(result, test_string);
        }
        {
            let test_string = "a";
            let encoded = to_encoding(test_string.as_bytes());
            read_string(&mut encoded.as_slice(), &mut buffers, &mut result).unwrap();
            assert_eq!(result, test_string);
        }
        {
            let test_string = "abc";
            let encoded = to_encoding(test_string.as_bytes());
            read_string(&mut encoded.as_slice(), &mut buffers, &mut result).unwrap();
            assert_eq!(result, test_string);
        }
        {
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
