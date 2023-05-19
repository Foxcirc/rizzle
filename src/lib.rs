
/*
* Interface to deezers public and private API.
* This library allows downloading songs, discovering music etc.
*/

#[cfg(test)]
mod test;

mod error;
mod util;

use blowfish::Blowfish;
use cipher::{KeyInit, BlockEncrypt, BlockDecrypt};
use generic_array::GenericArray;
use serde::Deserialize;
use serde_json::{Value, json};
use tinyvec::{ArrayVec, SliceVec};
use util::OptionToResult;

use std::{sync::{Arc, Mutex}, iter::{repeat, zip}, io::{Read, self}};

use error::Error;

#[derive(Clone)]
pub(crate) struct AuthMiddleware {
    pub(crate) user_agent: String,
    pub(crate) info: Arc<Mutex<Credentials<String>>>,
}

impl ureq::Middleware for AuthMiddleware {

    fn handle(&self, request: ureq::Request, next: ureq::MiddlewareNext) -> Result<ureq::Response, ureq::Error> {

        let info = self.info.lock().expect("todo: lock failed");

        next.handle(
            request
                .set("Accept", "*/*")
                .set("Cache-Control", "no-cache")
                .set("User-Agent", &self.user_agent)
                .set("Connection", "keep-alive")
                .set("DNT", "1")
                .set("Cookie", &format!("sid={}; arl={}", info.sid, info.arl))
        )

    }

}

#[derive(Debug)]
pub struct Credentials<T> {
    pub sid: T,
    pub arl: T,
}

pub struct Session {
    agent: ureq::Agent,
    info: Arc<Mutex<Credentials<String>>>,
    api_token: String,
}

impl Session {

    pub fn new<T: AsRef<str>>(info: Credentials<T>) -> Result<Self, Error> {

        let info = Arc::new(Mutex::new(Credentials {
            sid: info.sid.as_ref().to_string(),
            arl: info.arl.as_ref().to_string(),
        }));

        let middleware = AuthMiddleware {
            user_agent: "Rizzle".to_string(),
            info: Arc::clone(&info)
        };

        let agent = ureq::AgentBuilder::new()
            .middleware(middleware)
            .build();

        let mut session = Self {
            agent,
            info,
            api_token: "".to_string(),
        };

        // get the session id and api token from deezer
        // so we can use the full gw-light api

        let result = session.gw_light_query("deezer.getUserData", json!({}))?;
        let api_token = result["results"]["checkForm"].as_str().some()?.to_string();

        session.api_token = api_token;

        Ok(session)
            
    }

    pub fn search(&self, query: &str) -> Result<Outcome, Error> {

        let mut outcome = Outcome::default();

        let response = self.gw_light_query("deezer.pageSearch", json!({
            "query": query,
            "start": 0,
            "nb": 40,
            "suggest": true,
            "artist_suggest": true,
            "top_tracks": true
        }))?;

        // todo: make this all an Outcome::deserialize

        let result = &response["results"];

        let is_corrected = result["AUTOCORRECT"].as_bool().some()?;
        if is_corrected {
            outcome.corrected = Some(result["REVISED_QUERY"].as_str().some()?.to_string());
        }

        let top = result["TOP_RESULT"].as_array().some()?;
        if let Some(elem) = top.get(0) {
            let artist = Artist::deserialize(elem)?;
            outcome.top = Some(artist);
        }

        let tracks = result["TRACK"]["data"].as_array().some()?;
        for elem in tracks {
            let track = Track::deserialize(elem)?;
            outcome.tracks.push(track);
        }

        let artists = result["ARTIST"]["data"].as_array().some()?;
        for elem in artists {
            let artist = Artist::deserialize(elem)?;
            outcome.artists.push(artist);
        }
        
        Ok(outcome)

    }

    pub fn details(artist: &Artist) -> Result<(), Error> {

        Ok(())
        
    }

    pub fn stream(&self, track: &Track) -> Result<TrackStream, Error> {

        let song_quality = 1;

        let url_key = generate_url_key(track, song_quality);

        let url = format!("https://e-cdns-proxy-{}.dzcdn.net/mobile/1/{}", &track.md5_origin[0..1], url_key);

        let reader = self.agent.get(&url).call().map_err(|err| Error::CannotDownload(err))?.into_reader();

        let blowfish_key = generate_blowfish_key(track);
        let blowfish = Blowfish::new_from_slice(blowfish_key.as_bytes()).expect("Invalid key for Blowfish");

        Ok(TrackStream {
            reader,
            blowfish,
            count: 0,
            storage: ArrayVec::default(),
        })

    }

    pub fn end(self) -> Credentials<String> {

        drop(self.agent);

        let info = Arc::try_unwrap(self.info)
            .expect("Can't destroy `info` Arc")
            .into_inner()
            .expect("Can't destroy `info` Mutex");

        info

    }

    fn gw_light_query(&self, method: &str, body: Value) -> Result<Value, Error> {

        let body_str = body.to_string();

        let result: Value = self.agent.post("https://www.deezer.com/ajax/gw-light.php")
            .query("method", method)
            .query("input", "3")
            .query("api_version", "1.0")
            .query("api_token", &self.api_token)
            .query("cid", "943306354") // Math.floor(1000000000 * Math.random())
            .set("Content-Length", &body_str.len().to_string())
            .send_string(&body_str)?
            .into_json()?;

        Ok(result)

    }

}

fn generate_blowfish_key(track_details: &Track) -> String {

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

fn generate_url_key(track_details: &Track, quality: usize) -> String {

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

pub struct TrackStream {
    reader: Box<dyn Read>,
    blowfish: Blowfish,
    count: usize,
    storage: ArrayVec<[u8; 2048]>,
}

impl Read for TrackStream {

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
        // requested, this block is not encrypted
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

/// Basically the implementation from std but modified to return the number
/// of bytes that were read on EOF.
fn try_read_exact(mut this: impl Read, mut buff: &mut [u8]) -> ReadExact {
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

#[derive(Debug, Clone, Default)]
pub struct Outcome {
    pub top: Option<Artist>,
    pub tracks: Vec<Track>,
    pub artists: Vec<Artist>,
    pub corrected: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Track {
    #[serde(rename = "SNG_ID", deserialize_with = "str_to_u64")]
    pub id: u64,
    #[serde(rename = "SNG_TITLE")]
    pub name: String,
    #[serde(rename = "MD5_ORIGIN")]
    pub md5_origin: String,
    #[serde(rename = "MEDIA_VERSION", deserialize_with = "str_to_u64")]
    pub media_version: u64,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Artist {
    #[serde(rename = "ART_ID", deserialize_with = "str_to_u64")]
    pub id: u64,
    #[serde(rename = "ART_NAME")]
    pub name: String,
}

fn str_to_u64<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let s: &str = Deserialize::deserialize(deserializer)?;
    Ok(s.parse::<u64>().unwrap())
}

