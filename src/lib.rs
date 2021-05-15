use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Read;

#[derive(Debug, Default, Clone)]
pub struct SaveFile {
    save_header: i32,
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

        self.save_header = file.read_i32::<LittleEndian>()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::SaveFile;
    use std::fs::{read, File};

    #[test]
    fn parse() {
        let mut file = File::open("test_files/new_world.sav").unwrap();
        let mut save_file = SaveFile::default();
        save_file.parse(&mut file).unwrap();
        dbg!(&save_file);
    }
}
