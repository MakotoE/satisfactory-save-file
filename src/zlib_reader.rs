use super::*;
use flate2::read::ZlibDecoder;
use std::io::Take;

/// Reads the ZLib compressed parts of the file.
#[derive(Debug)]
pub struct ChunkedZLibReader<R>
where
    R: Read + Seek,
{
    decoder: Option<ZlibDecoder<Take<R>>>,
}

impl<R: Read + Seek> ChunkedZLibReader<R> {
    pub fn new(mut file: R) -> Result<Self> {
        let chunk_length = ChunkedZLibReader::read_header(&mut file)?;
        let mut decoder = ZlibDecoder::new(file.take(chunk_length));

        // Data length
        decoder.read_i32::<L>()?;

        Ok(Self {
            decoder: Some(decoder),
        })
    }

    fn read_header(file: &mut R) -> Result<u64> {
        let package_file_tag = file.read_i64::<L>()?;
        if package_file_tag != 0x9E2A83C1 {
            log::error!("unexpected package file tag: {}", package_file_tag);
        }
        let max_chunk_size = file.read_i64::<L>()?;
        if max_chunk_size != 0x20000 {
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
        let result = if let Some(decoder) = self.decoder.as_mut() {
            decoder.read(buf)
        } else {
            // This branch happens after read_header() returned UnexpectedEof below. We return 0 to
            // indicate end of file.
            return Ok(0);
        };

        if let Ok(bytes_read) = result {
            // End of chunk
            if bytes_read < buf.len() {
                let mut file = self.decoder.take().unwrap().into_inner().into_inner();

                let chunk_length = match ChunkedZLibReader::read_header(&mut file) {
                    Ok(n) => n,
                    Err(e) => {
                        if let Some(e) = e.downcast_ref::<std::io::Error>() {
                            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                                // If end of file is reached, attempting to read header returns
                                // UnexpectedEof
                                return Ok(bytes_read);
                            }
                        }
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
                    }
                };

                self.decoder = Some(ZlibDecoder::new(file.take(chunk_length)));

                if bytes_read == 0 {
                    return self.decoder.as_mut().unwrap().read(buf);
                }
            }
        }

        result
    }
}
