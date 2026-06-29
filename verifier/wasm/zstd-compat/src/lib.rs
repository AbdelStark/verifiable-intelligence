use std::io::{self, Read};

pub fn encode_all<R: Read>(mut source: R, level: i32) -> io::Result<Vec<u8>> {
    let mut input = Vec::new();
    source.read_to_end(&mut input)?;
    let compression_level = if level <= 0 {
        ruzstd::encoding::CompressionLevel::Uncompressed
    } else {
        ruzstd::encoding::CompressionLevel::Fastest
    };
    Ok(ruzstd::encoding::compress_to_vec(&input[..], compression_level))
}

pub fn decode_all<R: Read>(source: R) -> io::Result<Vec<u8>> {
    let mut decoder = ruzstd::decoding::StreamingDecoder::new(source)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let mut output = Vec::new();
    decoder.read_to_end(&mut output)?;
    Ok(output)
}
