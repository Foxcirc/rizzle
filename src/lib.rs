
/*
* Interface to deezers public and private API.
* This library allows downloading songs, discovering music etc.
*/

#[cfg(test)]
mod test;

mod error;
mod util;
mod decrypt;

use serde_derive::Deserialize;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};

use error::Error;
use util::OptionToResult;
use decrypt::*;

pub(crate) struct AuthMiddleware {
    pub(crate) user_agent: String,
    pub(crate) info: CredentialsPartial,
}

impl ureq::Middleware for AuthMiddleware {

    fn handle(&self, request: ureq::Request, next: ureq::MiddlewareNext) -> Result<ureq::Response, ureq::Error> {

        next.handle(
            request
                .set("Accept", "*/*")
                .set("Cache-Control", "no-cache")
                .set("User-Agent", &self.user_agent)
                .set("Connection", "keep-alive")
                .set("DNT", "1")
                .set("Cookie", &format!("sid={}; arl={}", self.info.sid, self.info.arl))
        )

    }

}

#[derive(Debug)]
pub(crate) struct CredentialsPartial {
    pub sid: String,
    pub arl: String,
}

#[derive(Debug)]
pub struct Credentials {
    pub sid: String,
    pub arl: String,
    pub api_token: String,
}

pub struct Session {
    agent: ureq::Agent,
    api_token: String,
}

impl Session {

    pub fn new(cred: Credentials) -> Self {

        let info = CredentialsPartial {
            sid: cred.sid.clone(),
            arl: cred.arl.clone(),
        };

        let middleware = AuthMiddleware {
            user_agent: "Rizzle".to_string(),
            info,
        };

        let agent = ureq::AgentBuilder::new()
            .middleware(middleware)
            .build();

        Self {
            agent,
            api_token: cred.api_token,
        }
            
    }

    pub fn search(&mut self, query: &str) -> Result<SearchResult, Error> {

        let result = self.gw_light_query("deezer.pageSearch", json!({
            "query": query,
            "start": 0,
            "nb": 40,
            "suggest": true,
            "artist_suggest": true,
            "top_tracks": true
        }))?;

        let search_result = Deserialize::deserialize(result)?;
        
        Ok(search_result)

    }

    pub fn details<'de, D: Details<'de>>(&mut self, item: &D) -> Result<D::Output, Error> {

        let query = item.details_query();
        let result = self.gw_light_query(query.method, query.body)?;

        let details = D::Output::deserialize(result)?;

        Ok(details)
        
    }

    pub fn stream_mp3(&self, track: &Track) -> Result<Mp3Stream, Error> {

        let song_quality = 1;

        let url_key = generate_url_key(track, song_quality);
        let blowfish_key = generate_blowfish_key(track);

        let url = format!("https://e-cdns-proxy-{}.dzcdn.net/mobile/1/{}", &track.md5_origin[0..1], url_key);
        let reader = self.agent.get(&url).call().map_err(|err| Error::CannotDownload(err))?.into_reader();

        Ok(Mp3Stream::new(reader, blowfish_key.as_bytes()))

    }


    #[cfg(feature = "decode")]
    pub fn stream_raw(&self, track: &Track) -> Result<RawStream, Error> {

        let stream = self.stream_mp3(track)?;
        Ok(RawStream::new(stream))

    }

    pub fn end(self) -> String {

        drop(self.agent);

        self.api_token

    }

    fn gw_light_query(&mut self, method: &str, body: Value) -> Result<Value, Error> {

        let mut response = self.gw_light_query_raw(method, &body)?;

        // If we get a CSRF error the Api token may be out of date
        if has_csrf_token_error(&response) {

            // Update the Api token
            // note: the license_token is present in the getUserData request USER/OPTIONS field
            // it is used for streaming hq songs with a paid acc I LOVE MY LIFE I FOUND IT GOD YEA
            // note: it has an expiration timestamp
            let user_data = self.gw_light_query("deezer.getUserData", json!({}))?;
            let api_token = user_data["checkForm"].as_str().some()?.to_string();
            self.api_token = api_token;

            response = self.gw_light_query_raw(method, &body)?;

            if has_csrf_token_error(&response) {
                return Err(Error::InvalidCredentials)
            }

        }

        let result = response["results"].take();

        Ok(result)

    }

    fn gw_light_query_raw(&self, method: &str, body: &Value) -> Result<Value, Error> {

        let body_str = body.to_string();

        let result: Value = self.agent.post("https://www.deezer.com/ajax/gw-light.php")
            .query("method", method)
            .query("input", "3")
            .query("api_version", "1.0")
            .query("api_token", &self.api_token)
            .query("cid", "943306354") // deezer source: Math.floor(1000000000 * Math.random())
            .set("Content-Length", &body_str.len().to_string())
            .send_string(&body_str)?
            .into_json()?;

        Ok(result)

    }

}

fn has_csrf_token_error(value: &Value) -> bool {
        value["error"].as_object().filter(|obj| obj["VALID_TOKEN_REQUIRED"].as_str() == Some("Invalid CSRF token")).is_some()
}

pub struct DetailsQuery {
    pub(crate) method: &'static str,
    pub(crate) body: serde_json::Value,
}

pub trait Details<'de> {
    type Output: Deserialize<'de> + DeserializeOwned;
    fn details_query(&self) -> DetailsQuery;
}

impl<'de> Details<'de> for Artist {
    type Output = ArtistDetails;
    fn details_query(&self) -> DetailsQuery {
        DetailsQuery {
            method: "deezer.pageArtist",
            body: json!({"art_id": self.id.to_string(), "lang": "en", "tab": 0})
        }
    }
}

impl<'de> Details<'de> for Album {
    type Output = AlbumDetails;
    fn details_query(&self) -> DetailsQuery {
        DetailsQuery {
            method: "deezer.pageAlbum",
            body: json!({"alb_id": self.id.to_string(), "header": true, "lang": "en", "tab": 0})
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SearchResult {
    #[serde(rename = "TOP_RESULT", deserialize_with = "des_array_to_option")]
    pub top: Option<Artist>,
    #[serde(rename = "TRACK", deserialize_with = "des_after_data")]
    pub tracks: Vec<Track>,
    #[serde(rename = "ARTIST", deserialize_with = "des_after_data")]
    pub artists: Vec<Artist>,
    #[serde(rename = "ALBUM", deserialize_with = "des_after_data")]
    pub albums: Vec<Album>,
    #[serde(default, rename = "REVISED_QUERY")]
    pub revised_query: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Track {
    #[serde(rename = "SNG_ID", deserialize_with = "des_parse_str")]
    pub id: u64,
    #[serde(rename = "SNG_TITLE")]
    pub name: String,
    #[serde(rename = "ARTISTS")]
    pub artists: Vec<Artist>,
    #[serde(rename = "MD5_ORIGIN")]
    md5_origin: String,
    #[serde(rename = "MEDIA_VERSION", deserialize_with = "des_parse_str")]
    media_version: u64,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Artist {
    #[serde(rename = "ART_ID", deserialize_with = "des_parse_str")]
    pub id: u64,
    #[serde(rename = "ART_NAME")]
    pub name: String,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct ArtistDetails {
    #[serde(rename = "ALBUMS", deserialize_with = "des_after_data")]
    pub albums: Vec<Album>,
    #[serde(rename = "TOP", deserialize_with = "des_after_data")]
    pub top_tracks: Vec<Track>,
    // #[serde(rename = "HIGHLIGHT", flatten)]
    // pub highlight: Highlights,
    #[serde(rename = "RELATED_ARTISTS", deserialize_with = "des_after_data")]
    pub related: Vec<Artist>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Album {
    #[serde(rename = "ALB_ID", deserialize_with = "des_parse_str")]
    pub id: u64,
    #[serde(rename = "ALB_TITLE")]
    pub name: String,
    #[serde(rename = "PHYSICAL_RELEASE_DATE")]
    pub date: String,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct AlbumDetails {
    #[serde(rename = "SONGS", deserialize_with = "des_after_data")]
    pub tracks: Vec<Track>,
}

fn des_parse_str<'de, D: serde::Deserializer<'de>, T: std::str::FromStr>(deserializer: D) -> Result<T, D::Error> {
    let string: String = Deserialize::deserialize(deserializer)?;
    let res = match string.parse() {
        Ok(val) => val,
        Err(..) => return Err(serde::de::Error::invalid_value(serde::de::Unexpected::Other(&string), &&format!("string, parsable as {}", std::any::type_name::<T>())[..]))
    };
    Ok(res)
}

fn des_after_data<'de, D: serde::Deserializer<'de>, T: DeserializeOwned>(deserializer: D) -> Result<T, D::Error> {
    let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
    let data = &value["data"];
    serde_json::from_value(data.to_owned()).map_err(serde::de::Error::custom)
}

fn des_array_to_option<'de, D: serde::Deserializer<'de>, T: DeserializeOwned>(deserializer: D) -> Result<Option<T>, D::Error> {
    let mut array: Vec<serde_json::Value> = Deserialize::deserialize(deserializer)?;
    let elem = array.drain(..).next();
    match elem {
        Some(value) => Ok(Some(serde_json::from_value(value).map_err(serde::de::Error::custom)?)),
        None => Ok(None),
    }
}

