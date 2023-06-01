
/*
* Interface to deezers public and private API.
* This library allows downloading songs, discovering music etc.
*/

#[cfg(test)]
mod test;

mod error;
mod util;
mod decrypt;

use std::sync::{Arc, Mutex};

use serde_derive::Deserialize;
use serde::{de::{DeserializeOwned, Deserialize}, Deserializer};
use serde_json::{Value, json};

pub use error::Error;
pub use decrypt::*;

pub(crate) struct AuthMiddleware {
    pub(crate) user_agent: String,
    pub(crate) info: Credentials,
    pub(crate) license_token: Arc<Mutex<String>>,
}

impl ureq::Middleware for AuthMiddleware {

    fn handle(&self, request: ureq::Request, next: ureq::MiddlewareNext) -> Result<ureq::Response, ureq::Error> {

        let license_token = self.license_token.lock().expect("Cannot lock `license_token Mutex");

        next.handle(
            request
                .set("Accept", "*/*")
                .set("Cache-Control", "no-cache")
                .set("User-Agent", &self.user_agent)
                .set("Connection", "keep-alive")
                .set("DNT", "1")
                .set("Cookie", &format!("sid={}; arl={}; license_token={}", self.info.sid, self.info.arl, license_token))
        )

    }

}

#[derive(Debug, Default, Deserialize)]
pub struct Credentials {
    pub sid: String,
    pub arl: String,
}

pub struct Session {
    agent: ureq::Agent,
    user: User,
}

impl Session {

    pub fn new(info: Credentials) -> Result<Self, Error> {

        let license_token = Arc::new(Mutex::new("".to_string()));

        let middleware = AuthMiddleware {
            user_agent: "Rizzle".to_string(),
            info,
            license_token: Arc::clone(&license_token),
        };

        let agent = ureq::AgentBuilder::new()
            .middleware(middleware)
            .build();

        let user_raw = Self::gw_light_query_raw(&agent, "", "deezer.getUserData", json!({}))?;
        let user: User = Deserialize::deserialize(user_raw)?;

        *license_token.lock().expect("Cannot lock `license_token` Mutex") = user.license_token.clone();

        Ok(Self {
            agent,
            user,
        })
            
    }

    pub fn user(&self) -> Result<User, Error> {
        Ok(Clone::clone(&self.user))
    }

    pub fn search(&self, query: &str) -> Result<SearchResult, Error> {

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

    pub fn details<'de, O: Deserialize<'de>, D: Details<'de, O>>(&self, item: &D) -> Result<O, Error> {

        let query = item.details_query();
        let result = match query.api {
            DetailsApi::GwLightApi(method) => self.gw_light_query(method, query.body)?,
            DetailsApi::PipeApi => self.pipe_query(query.body)?,
        };

        let details = O::deserialize(result)?;

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

    fn pipe_query(&self, body: Value) -> Result<Value, Error> {
        Self::pipe_query_raw(&self.agent, body)
    }

    fn pipe_query_raw(agent: &ureq::Agent, body: Value) -> Result<Value, Error> {
        
        let body_str = body.to_string();

        let mut response: Value = agent.post("https://pipe.deezer.com/api")
            .set("Content-Length", &body_str.len().to_string())
            .send_string(&body_str)?
            .into_json()?;

        let result = response["data"].take();

        Ok(result)

    }

    fn gw_light_query(&self, method: &str, body: Value) -> Result<Value, Error> {
        Self::gw_light_query_raw(&self.agent, &self.user.api_token, method, body)
    }

    fn gw_light_query_raw(agent: &ureq::Agent, api_token: &str, method: &str, body: Value) -> Result<Value, Error> {

        let body_str = body.to_string();

        let mut response: Value = agent.post("https://www.deezer.com/ajax/gw-light.php")
            .query("method", method)
            .query("input", "3")
            .query("api_version", "1.0")
            .query("api_token", api_token)
            .query("cid", "943306354") // deezer source: Math.floor(1000000000 * Math.random())
            .set("Content-Length", &body_str.len().to_string())
            .send_string(&body_str)?
            .into_json()?;

        // If we get a CSRF error the Api token may be out of date
        if Self::has_csrf_token_error(&response) {
            return Err(Error::InvalidCredentials)
        }

        let result = response["results"].take();

        Ok(result)

    }

    fn has_csrf_token_error(value: &Value) -> bool {
        value["error"].as_object().filter(|obj| obj["VALID_TOKEN_REQUIRED"].as_str() == Some("Invalid CSRF token")).is_some()
    }

}


pub struct DetailsQuery {
    pub(crate) api: DetailsApi,
    pub(crate) body: serde_json::Value,
}

pub enum DetailsApi {
    PipeApi,
    GwLightApi(&'static str),
}

pub trait Details<'de, O> {
    fn details_query(&self) -> DetailsQuery;
}

impl<'de> Details<'de, ArtistDetails> for Artist {
    fn details_query(&self) -> DetailsQuery {
        DetailsQuery {
            api: DetailsApi::GwLightApi("deezer.pageArtist"),
            body: json!({"art_id": self.id.to_string(), "lang": "en", "tab": 0})
        }
    }
}

impl<'de> Details<'de, AlbumDetails> for Album {
    fn details_query(&self) -> DetailsQuery {
        DetailsQuery {
            api: DetailsApi::GwLightApi("deezer.pageAlbum"),
            body: json!({"alb_id": self.id.to_string(), "header": true, "lang": "en", "tab": 0})
        }
    }
}

impl<'de> Details<'de, PlaylistDetails> for Playlist {
    fn details_query(&self) -> DetailsQuery {
        DetailsQuery {
            api: DetailsApi::GwLightApi("deezer.pagePlaylist"), // todo: make "nb" be changable
            body: json!({ "header": true, "lang": "en", "nb": 2000, "playlist_id": self.id.to_string(), "start": 0, "tab": 0, "tags": true })
        }
    }
}

impl<'de> Details<'de, UserLibrary> for User {
    fn details_query(&self) -> DetailsQuery {
        DetailsQuery {
            api: DetailsApi::GwLightApi("deezer.userMenu"),
            body: json!({})
        }
    }
}

impl<'de> Details<'de, UserFamily> for User {
    fn details_query(&self) -> DetailsQuery {
        DetailsQuery {
            api: DetailsApi::GwLightApi("deezer.getChildAccounts"),
            body: json!({})
        }
    }
}

impl<'de> Details<'de, TrackLyrics> for Track {
    fn details_query(&self) -> DetailsQuery {
        DetailsQuery {
            api: DetailsApi::PipeApi,
            // body: json!({"operationName": "SynchronizedTrackLyrics", "query": "query SynchronizedTrackLyrics($trackId: String!) { track(trackId: $trackId) { ...SynchronizedTrackLyrics } } fragment SynchronizedTrackLyrics on Track { id lyrics { ...Lyrics } } fragment Lyrics on Lyrics { id copyright text writers synchronizedLines { ...LyricsSynchronizedLines } } fragment LyricsSynchronizedLines on LyricsSynchronizedLine { lrcTimestamp line lineTranslated milliseconds duration } ", "variables": { "trackId": self.id.to_string() } }),
            body: json!({
                "operationName": "SynchronizedTrackLyrics",
                "query": "query SynchronizedTrackLyrics($trackId: String!) {\n  track(trackId: $trackId) {\n    ...SynchronizedTrackLyrics\n    __typename\n  }\n}\n\nfragment SynchronizedTrackLyrics on Track {\n  id\n  lyrics {\n    ...Lyrics\n    __typename\n  }\n  album {\n    cover {\n      small: urls(pictureRequest: {width: 100, height: 100})\n      medium: urls(pictureRequest: {width: 264, height: 264})\n      large: urls(pictureRequest: {width: 800, height: 800})\n      explicitStatus\n      __typename\n    }\n    __typename\n  }\n  __typename\n}\n\nfragment Lyrics on Lyrics {\n  id\n  copyright\n  text\n  writers\n  synchronizedLines {\n    ...LyricsSynchronizedLines\n    __typename\n  }\n  __typename\n}\n\nfragment LyricsSynchronizedLines on LyricsSynchronizedLine {\n  lrcTimestamp\n  line\n  lineTranslated\n  milliseconds\n  duration\n  __typename\n}",
                "variables": {
                    "trackId": self.id,
                }
            })
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SmallUser {
    #[serde(rename = "USER_ID", deserialize_with = "des_parse_str")]
    pub id: usize,
    #[serde(rename = "BLOG_NAME")]
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct UserFamily {
    pub users: Vec<SmallUser>,
}

impl<'de> Deserialize<'de> for UserFamily {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value: Value = Deserialize::deserialize(deserializer)?;
        let mut users = Vec::new();
        let items = match value.as_array() { Some(val) => val, None => return Err(serde::de::Error::custom("UserFamily: Expected array of users")) };
        for item in items {
            users.push(Deserialize::deserialize(item).map_err(|_| serde::de::Error::custom("UserFamily: Not a valid User entry"))?)
        }
        Ok(UserFamily { users })
    }
}

#[derive(Debug, Clone, Default)]
pub struct UserLibrary {
    pub playlists: Vec<Playlist>,
    pub history: Vec<String>,
    // todo: add "notifications"
}

impl<'de> Deserialize<'de> for UserLibrary {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut value: Value = Deserialize::deserialize(deserializer)?;
        let playlists = Deserialize::deserialize(value["PLAYLISTS"].take()).map_err(|_| serde::de::Error::missing_field("PLAYLISTS"))?;
        let history_raw: Vec<Value> = Deserialize::deserialize(value["SEARCH_HISTORY"].take()).map_err(|_| serde::de::Error::missing_field("SEARCH_HISTORY"))?;
        let mut history = Vec::new();
        for item in history_raw {
            history.push(match item["query"].as_str() { Some(val) => val.to_string(), None => return Err(serde::de::Error::missing_field("query")) })
        }
        Ok(UserLibrary { playlists, history })
    }
}

#[derive(Debug, Clone, Default)]
pub struct User {
    api_token: String,
    license_token: String,
    pub id: usize,
    pub created: String,
    pub name: String,
    pub multiaccount: bool,

}

impl<'de> Deserialize<'de> for User {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value: Value = Deserialize::deserialize(deserializer)?;
        let api_token = match value["checkForm"].as_str() { Some(val) => val.to_string(), None => return Err(serde::de::Error::missing_field("checkForm")) };
        let license_token = match value["USER"]["OPTIONS"]["license_token"].as_str() { Some(val) => val.to_string(), None => return Err(serde::de::Error::missing_field("license_token")) };
        let id = match value["USER"]["USER_ID"].as_u64() { Some(val) => val as usize, None => return Err(serde::de::Error::missing_field("USER_ID")) };
        let created = match value["USER"]["INSCRIPTION_DATE"].as_str() { Some(val) => val.to_string(), None => return Err(serde::de::Error::missing_field("INSCRIPTION_DATE")) };
        let name = match value["USER"]["BLOG_NAME"].as_str() { Some(val) => val.to_string(), None => return Err(serde::de::Error::missing_field("BLOG_NAME")) };
        let multiaccount = match value["USER"]["MULTI_ACCOUNT"]["enabled"].as_bool() { Some(val) => val, None => false };
        Ok(User { api_token, license_token, id, created, name, multiaccount })
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
    #[serde(rename = "PLAYLIST", deserialize_with = "des_after_data")]
    pub playlists: Vec<Playlist>,
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

#[derive(Debug, Default, Clone)]
pub struct TrackLyrics {

}

impl<'de> Deserialize<'de> for TrackLyrics {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(TrackLyrics {})
    }
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
    pub release_date: String,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct AlbumDetails {
    #[serde(rename = "SONGS", deserialize_with = "des_after_data")]
    pub tracks: Vec<Track>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Playlist {
    #[serde(rename = "PLAYLIST_ID", deserialize_with = "des_parse_str")]
    pub id: u64,
    #[serde(rename = "TITLE")]
    pub name: String,
    #[serde(rename = "DATE_MOD")]
    pub last_modified: String,
    #[serde(rename = "NB_SONG")]
    pub songs: usize,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct PlaylistDetails {
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

