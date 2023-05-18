
/*
* Interface to deezers public and private API.
* This library allows downloading songs, discovering music etc.
*/

#[cfg(test)]
mod test;

mod error;
mod util;

use serde::Deserialize;
use serde_json::{Value, json};
use util::OptionToResult;

use std::sync::{Arc, Mutex};

use error::Error;

#[derive(Clone)]
pub(crate) struct AuthMiddleware {
    pub(crate) user_agent: String,
    pub(crate) info: Arc<Mutex<DeezerAuth<String>>>,
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
pub struct DeezerAuth<T> {
    pub sid: T,
    pub arl: T,
}

pub struct Session {
    agent: ureq::Agent,
    info: Arc<Mutex<DeezerAuth<String>>>,
    api_token: String,
}

impl Session {

    pub fn new<T: AsRef<str>>(info: DeezerAuth<T>) -> Result<Self, Error> {

        let info = Arc::new(Mutex::new(DeezerAuth {
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

        // get the session id and api token from deezer
        // so we can use the full gw-light api

        let result: Value = agent.post("https://www.deezer.com/ajax/gw-light.php")
            .query("method", "deezer.getUserData")
            .query("input", "3")
            .query("api_version", "1.0")
            .query("api_token", "")
            .query("cid", "132524931") // Math.floor(1000000000 * Math.random())
            .set("Content-Length", "2")
            .send_string("{}")?
            .into_json()?;

        let api_token = result["results"]["checkForm"].as_str().some()?.to_string();

        println!("api token: {}", api_token);

        Ok(Self {
            agent,
            info,
            api_token,
        })
            
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

    pub fn stream(&self, track: &Track) -> Result<TrackStream, Error> {

        let song_query = json!({
            "sng_ids": [track.id.to_string()],
        });

        let response = self.gw_light_query("song.getListData", song_query)?;

        let result = &response["results"]["data"][0];
        let track_details = TrackDetails::deserialize(result)?;



        Ok(TrackStream {})

    }

    pub fn end(self) -> DeezerAuth<String> {

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

pub struct TrackStream {

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
}

#[derive(Debug, Default, Clone, Deserialize)]
pub(crate) struct TrackDetails {

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

