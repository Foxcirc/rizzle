
use std::{io::{self, Read}, iter::{zip, repeat}};

use cipher::{KeyInit, BlockEncrypt, BlockDecrypt};
use futures_lite::io::BlockOn;
use generic_array::GenericArray;
use tinyvec::{SliceVec, ArrayVec};

use crate::Track;

pub(crate) fn generate_blowfish_key(track_details: &Track) -> String {

    let key = b"g4el58wc0zvf9na1";

    let id_md5 = md5::compute(track_details.id.to_string().as_bytes());
    let id_md5_str = hex::encode(id_md5.0);
    let id_md5_bytes = id_md5_str.as_bytes();

    let mut result = String::with_capacity(16);

    for idx in 0..16 {
        let value = id_md5_bytes[idx] ^ id_md5_bytes[idx + 16] ^ key[idx];
        result.push(value as char);
    }

    result

}

pub(crate) fn generate_url_key(track_details: &Track, quality: usize) -> String {

    let mut data = Vec::new(); // todo: use smallvec / tinyvec

    data.extend_from_slice(track_details.md5_origin.as_bytes());
    data.extend_from_slice(b"\xa4");
    data.extend_from_slice(quality.to_string().as_bytes());
    data.extend_from_slice(b"\xa4");
    data.extend_from_slice(track_details.id.to_string().as_bytes());
    data.extend_from_slice(b"\xa4");
    data.extend_from_slice(track_details.media_version.to_string().as_bytes());

    let data_md5 = md5::compute(&data);
    let data_md5_str = hex::encode(data_md5.0);

    let mut data_full = Vec::new(); // todo: use smallvec / tinyvec

    data_full.extend_from_slice(data_md5_str.as_bytes());
    data_full.extend_from_slice(b"\xa4");
    data_full.extend_from_slice(&data);
    data_full.extend_from_slice(b"\xa4");

    let missing = data_full.len() % 16;
    if missing != 0 {
        data_full.extend(repeat(b'\0').take(16 - missing))
    }
    
    let key = b"jo6aey6haid2Teih";
    let cipher = aes::Aes128Enc::new(key.into());

    for block in data_full.chunks_mut(16).map(|chunk| chunk.into()) {
        cipher.encrypt_block(block);
    }

    let encoded = hex::encode(data_full);

    return encoded

}

pub struct Mp3Stream {
    reader: BlockOn<rtv::BodyReader>, // we can just use BlockOn since rtv provides it's own reactor
    blowfish: blowfish::Blowfish,
    count: usize,
    storage: ArrayVec<[u8; 2048]>,
}

impl Mp3Stream {

    pub(crate) fn new(reader: rtv::BodyReader, key: &[u8]) -> Self {
        Self {
            reader: BlockOn::new(reader),
            blowfish: blowfish::Blowfish::new_from_slice(key).expect("Invalid blowfish key"),
            count: 0,
            storage: Default::default(),
        }
    }

}

impl<'a> Read for Mp3Stream {

    fn read(&mut self, buff: &mut [u8]) -> std::io::Result<usize> {

        // good luck understanding this

        let mut dest = SliceVec::from(buff);
        dest.clear();

        let len = dest.capacity();
        let bytes_read;

        // if we have more bytes stored then requested just return them
        // and shrink the storage
        if len <= self.storage.len() {
            dest.extend(self.storage.drain(..len));
            return Ok(len);
        }

        // calculate how many bytes we need to read after using 
        // the stored ones
        let new_len = len - self.storage.len();

        // calculate how many bytes we need to read in order to always
        // read on a 2048 byte block boundry
        let to_read = new_len + (2048 - new_len % 2048);

        dest.extend(self.storage.drain(..));

        // read the data, if there is less data on the reader left then
        // requested (the last chunk), this block is not encrypted
        let mut data = vec![0; to_read];
        match try_read_exact(&mut self.reader, &mut data) {
            ReadExact::Ok => bytes_read = len,
            ReadExact::Eof(val) => bytes_read = val,
            ReadExact::Err(err) => return Err(err),
        };

        // decrypt all blocks that need to be decrypted
        for chunk in data.chunks_mut(2048) {
            if chunk.len() == 2048 && self.count % 3 == 0 {
                // note: this is a manual implementation of blowfish cbc mode
                // (took way too long to figure out)
                let mut cbc_xor = *b"\x00\x01\x02\x03\x04\x05\x06\x07"; // magic iv
                let mut block_copy = [0; 8];
                for block in chunk.chunks_exact_mut(8) {
                    block_copy.copy_from_slice(block);
                    self.blowfish.decrypt_block(GenericArray::from_mut_slice(block));
                    zip(block.iter_mut(), cbc_xor).for_each(|(byte, val)| *byte ^= val);
                    cbc_xor = block_copy;
                }
            }
            self.count += 1;
        }

        if bytes_read >= new_len {
            dest.extend(data.drain(..new_len));
            self.storage.extend(data);
        } else {
            dest.extend(data.drain(..bytes_read));
        }

        Ok(bytes_read)

    }

}

#[cfg(feature = "decode")]
pub struct RawStream {
    decoder: minimp3::Decoder<Mp3Stream>,
    storage: Vec<u8>,
}

#[cfg(feature = "decode")]
impl RawStream {

    pub(crate) fn new(stream: Mp3Stream) -> Self {
        Self {
            decoder: minimp3::Decoder::new(stream),
            storage: Vec::new(),
        }
    }

}

#[cfg(feature = "decode")]
impl Iterator for RawStream {

    type Item = io::Result<Vec<i16>>;

    fn next(&mut self) -> Option<Self::Item> {
        
        let value = match self.decoder.next_frame() {
            Ok(frame) => Ok(frame.data),
            Err(minimp3::Error::Eof) => return None,
            Err(minimp3::Error::Io(err)) => Err(err),
            Err(other) => Err(io::Error::new(io::ErrorKind::Other, format!("error during mp3 decoding: {}", other))),
        };

        Some(value)

    }

}

#[cfg(feature = "decode")]
impl Read for RawStream {

    fn read(&mut self, buff: &mut [u8]) -> io::Result<usize> {

        let mut to_copy = buff.len();

        // read as much as we can out of the storage

        let storage_len = self.storage.len().min(to_copy);
        buff[..storage_len].copy_from_slice(&self.storage[..storage_len]);
        self.storage.drain(..storage_len);
        to_copy -= storage_len;

        // read some frames

        let mut data: Vec<u8> = Vec::with_capacity(to_copy);

        let mut samples_read = 0;
        while (samples_read * 2) < to_copy {

            let frame = match self.decoder.next_frame() {
                Ok(value) => value,
                Err(minimp3::Error::Eof) => break,
                Err(minimp3::Error::Io(err)) => return Err(err),
                Err(other) => return Err(io::Error::new(io::ErrorKind::Other, format!("error during mp3 decoding: {}", other))),
            };

            samples_read += frame.data.len();

            data.extend(frame.data.into_iter().map(|packet| packet.to_ne_bytes()).flatten());

        }

        let data_len = data.len().min(to_copy);
        buff[storage_len..storage_len + data_len].copy_from_slice(&data[..data_len]);

        self.storage.extend_from_slice(&data[data_len..]);

        Ok(storage_len + data_len)

    }    

}

/// Basically the implementation from std but modified to return the number
/// of bytes that were read on EOF.
fn try_read_exact<R: Read>(mut this: R, mut buff: &mut [u8]) -> ReadExact {
    let original_len = buff.len();
    while !buff.is_empty() {
        match this.read(buff) {
            Ok(0) => break,
            Ok(bytes_read) => {
                let temp = buff;
                buff = &mut temp[bytes_read..];
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => return ReadExact::Err(err),
        }
    }
    if !buff.is_empty() {
        ReadExact::Eof(original_len - buff.len())
    } else {
        ReadExact::Ok
    }
}

enum ReadExact {
    Ok,
    Eof(usize),
    Err(io::Error),
}

