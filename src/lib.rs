
/*
* Interface to deezers public and private API.
* This library allows downloading songs, discovering music, searching etc.
*/

#[cfg(test)]
mod test;

mod error;
mod decrypt;

use serde_derive::Deserialize;
use serde::{de::{DeserializeOwned, Deserialize}, Deserializer};
use serde_json::{Value as JsonValue, json};

pub use error::Error;
pub use decrypt::*;

#[derive(Debug, Default, Deserialize)]
pub struct UserInfo {
    pub sid: String,
    pub arl: String,
    #[serde(default)]
    pub user_agent: String,
}

pub struct Session {
    client: rtv::SimpleClient,
    middleware: Middleware,
    user: User,
}

impl Session {

    pub async fn new(info: UserInfo) -> Result<Self, Error> {

        // Todo: If UserInfo is empty, Deezer will automatically use a free account I think
        // maybe add support for that (no BLOG_NAME will be present etc.)

        let client = rtv::SimpleClient::new()?;

        let middleware = Middleware {
            user_agent: info.user_agent,
            arl: info.arl,
            sid: info.sid,
            license_token: String::new(),
            api_token: String::new(),
        };

        let mut session = Self {
            client,
            middleware,
            user: User::default(),
        };

        // initial query with the only authentication being the sid and arl
        let resp = session.gw_light_query("deezer.getUserData", json!({})).await?;
        let user: User = Deserialize::deserialize(resp)?;

        session.middleware.license_token = user.license_token.clone();
        session.middleware.api_token = user.api_token.clone();
        session.user = user;

        Ok(session)
            
    }

    pub fn user(&self) -> Result<User, Error> {
        Ok(Clone::clone(&self.user))
    }

    pub async fn search(&mut self, query: &str) -> Result<SearchResult, Error> {

        let result = self.gw_light_query("deezer.pageSearch", json!({
            "query": query,
            "start": 0,
            "nb": 40,
            "suggest": true,
            "artist_suggest": true,
            "top_tracks": true
        })).await?;

        let search_result = Deserialize::deserialize(result)?;
        
        Ok(search_result)

    }

    pub async fn details<'de, O: Deserialize<'de>, D: Details<'de, O>>(&mut self, item: &D) -> Result<O, Error> {

        let query = item.details_query();
        let result = match query.api {
            DetailsApi::GwLightApi(method) => self.gw_light_query(method, query.body).await?,
            DetailsApi::PipeApi => self.pipe_query(query.body).await?,
        };

        let details = O::deserialize(result)?;

        Ok(details)

    }

    pub async fn stream_mp3<'d>(&'d mut self, track: &Track) -> Result<Mp3Stream, Error> {

        let song_quality = 1;

        let url_key = generate_url_key(track, song_quality);
        let blowfish_key = generate_blowfish_key(track);

        let host = format!("e-cdns-proxy-{}.dzcdn.net", &track.md5_origin[0..1]);
        let path = format!("/mobile/1/{}", url_key);
        let req = rtv::Request::get().secure()
            .host(&host)
            .path(&path);

        let resp = self.client.stream(req).await?;

        Ok(Mp3Stream::new(resp.body, blowfish_key.as_bytes()))

    }


    #[cfg(feature = "decode")]
    pub async fn stream_raw<'d>(&'d mut self, track: &Track) -> Result<RawStream, Error> {

        let stream = self.stream_mp3(track).await?;
        Ok(RawStream::new(stream))

    }

    async fn pipe_query(&mut self, body: JsonValue) -> Result<JsonValue, Error> {
        
        let body_str = body.to_string();
        let req = rtv::Request::post().secure()
            .host("pipe.deezer.com")
            .path("/api")
            .send(&body_str);

        let req = self.middleware.decorate(req);

        let mut resp = JsonValue::from(self.client.send(req).await?.body);
        
        let result = resp["data"].take();

        Ok(result)

    }

    async fn gw_light_query(&mut self, method: &str, body: JsonValue) -> Result<JsonValue, Error> {

        let body_str = body.to_string();
        let req = rtv::Request::post().secure()
            .host("www.deezer.com")
            .path("/ajax/gw-light.php")
            .query("method", method)
            .query("input", "3")
            .query("api_version", "1.0")
            .query("api_token", &self.middleware.api_token)
            .query("cid", "94330654")
            .send(&body_str);

        let req = self.middleware.decorate(req);

        let resp = self.client.send(req).await?;
        let mut json = serde_json::from_slice(&resp.body)?;

        if Self::has_csrf_token_error(&json) {
            return Err(Error::InvalidCredentials)
        }

        let result = json["results"].take();

        Ok(result)

    }

    // todo: this doesn't work rn
    //       we get a "gateway error" if the request query params are wrong
    fn has_csrf_token_error(value: &JsonValue) -> bool {
        if let Some(error) = value.get("error") {
            if let Some(_msg) = error.as_object().and_then(|opt| opt.get("GATEWAY_ERROR")) {
                return true
            }
        }
        false
    }

}

/// Used to decorate a request with the necessery cookies
struct Middleware {
    user_agent: String,
    sid: String, // arl cookie
    arl: String, // sid cookie (session id)
    license_token: String,
    api_token: String,
}

impl Middleware {

    /// Decorate a request with the stored values
    fn decorate<'d>(&'d self, builder: rtv::RequestBuilder<'d>) -> rtv::RequestBuilder<'d> {
        builder
            .set("DNT", "1")
            .set("User-Agent", &self.user_agent)
            .cookie("arl", &self.arl)
            .cookie("sid", &self.sid)
            .cookie("license_token", &self.license_token)
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
        let value: JsonValue = Deserialize::deserialize(deserializer)?;
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
        let mut value: JsonValue = Deserialize::deserialize(deserializer)?;
        let playlists = Deserialize::deserialize(value["PLAYLISTS"].take()).map_err(|_| serde::de::Error::missing_field("PLAYLISTS"))?;
        let history_raw: Vec<JsonValue> = Deserialize::deserialize(value["SEARCH_HISTORY"].take()).map_err(|_| serde::de::Error::missing_field("SEARCH_HISTORY"))?;
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
        let value: JsonValue = Deserialize::deserialize(deserializer)?;
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
    fn deserialize<D: Deserializer<'de>>(_deserializer: D) -> Result<Self, D::Error> {
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

