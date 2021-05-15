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

        let mut buffer: Vec<u8> = Vec::new();

        self.save_header = file.read_i32::<L>()?;
        self.save_version = file.read_i32::<L>()?;
        self.build_version = file.read_i32::<L>()?;
        SaveFile::read_string(file, &mut buffer, &mut self.world_type);
        SaveFile::read_string(file, &mut buffer, &mut self.world_properties);
        self.play_time = file.read_i32::<L>()?;
        self.save_date = file.read_i64::<L>()?;
        self.session_visibility = file.read_u8()?;
        Ok(())
    }

    fn read_string<R>(file: &mut R, buffer: &mut Vec<u8>, s: &mut String) -> Result<()>
    where
        R: Read,
    {
        let length = (file.read_i32::<L>()? - 1) as usize; // This can be negative
        buffer.clear();
        buffer.resize(length, b'\0');
        file.read_exact(buffer);
        // Skip null char
        file.read_u8()?;
        s.clear();
        s.push_str(std::str::from_utf8(&buffer)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::SaveFile;
    use std::fs::File;

    #[test]
    fn parse() {
        let mut file = File::open("test_files/new_world.sav").unwrap();
        let mut save_file = SaveFile::default();
        save_file.parse(&mut file).unwrap();
        dbg!(&save_file);
    }
}
