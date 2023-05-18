
/*
* Interface to deezers public and private API.
* This library allows downloading songs, discovering music etc.
*/

#[cfg(test)]
mod test;

mod error;
mod util;

use cookie::Cookie;
use error::RizzleError;

pub struct Info<'a> {
    pub user_agent: &'a str,
    pub arl: &'a str,
    pub sid: &'a str,
}

#[derive(Clone)]
pub(crate) struct AuthMiddleware {
    user_agent: String,
    cookies: Vec<MiddlewareCookie>,
}

impl ureq::Middleware for AuthMiddleware {

    fn handle(&self, request: ureq::Request, next: ureq::MiddlewareNext) -> Result<ureq::Response, ureq::Error> {
        
        let mut cookies_str = String::with_capacity(128);

        for cookie in &self.cookies {
            cookies_str += &format!("{}={}; ", cookie.name, cookie.value);
        };

        next.handle(
            request
                .set("Cache-Control", "no-cache")
                .set("User-Agent", &self.user_agent)
                .set("Connection", "keep-alive")
                .set("DNT", "1")
                .set("Cookie", &cookies_str)
        )

    }

}

#[derive(Debug, Clone)]
pub(crate) struct MiddlewareCookie {
    name: String,
    value: String,
}

impl MiddlewareCookie {

    pub(crate) fn new(name: &str, value: &str) -> Self {
        Self {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    pub(crate) fn from_cookie(cookie: Cookie) -> Self {
        Self {
            name: cookie.name().to_string(),
            value: cookie.value().to_string(),
        }
    }

}

pub struct Session {
    agent: ureq::Agent,
}

impl Session {

    pub fn new(info: Info) -> Result<Self, RizzleError> {

        let middleware = AuthMiddleware {
            user_agent: info.user_agent.to_string(),
            cookies: vec![
                MiddlewareCookie::new("arl", info.arl),
                MiddlewareCookie::new("sid", info.sid),
            ],
        };

        let agent = ureq::AgentBuilder::new()
            .middleware(middleware)
            .build();

        let song_query = r#"{"sng_ids":["619949882"]}"#;

        let result = agent.post("https://www.deezer.com/ajax/gw-light.php")
            .query("method", "song.getListData")
            .query("input", "3")
            .query("api_version", "1.0")
            .query("api_token", "MaERfUjJO_BYO~YtidECCmjLpRYlsgmb")
            .query("cid", "696969696") // Math.floor(1000000000 * Math.random())
            .set("Content-Length", &song_query.len().to_string())
            .set("x-deezer-user", "1578041862")
            .send_string(song_query).expect("todo: req error")
            .into_string().expect("todo: cant get body as string");

        println!("{result}");

        todo!("impl lib");
            
    }

}

