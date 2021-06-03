#![no_main]
use libfuzzer_sys::fuzz_target;
use satisfactory_save_file::SaveFile;

fuzz_target!(|data: &[u8]| {
    let _ = SaveFile::parse(&mut &data.to_vec()[..]);
});