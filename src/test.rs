
use serde_json::Value;
use ureq::{Request, MiddlewareNext};
use crate::{util::OptionToResult, MiddlewareCookie, AuthMiddleware};

const DEEZER_API: &str = "https://api.deezer.com";

#[test]
fn discover_songs() -> anyhow::Result<()> {

    let agent = ureq::agent();

    let result: Value = agent.get(&format!("{DEEZER_API}/search/track?q=miraie")).call()?.into_json()?;

    for song in result["data"].as_array().some()?.iter() {
        println!("title: {}, artist: {}", song["title"], song["artist"]["name"]);
    }

    let song = &result["data"][4];

    for (key, _) in song.as_object().some()? {
        eprintln!("{key}");
    }

    Ok(())

}

const ARL_COOKIE: &str = "d13c0a4ecc37fe7f0986cc97cb16cc60e0dd96195b7193fac903a18c3f23e4e8355167a9367a04371eb446b33520f578c65b9bacb33d3fc1e755ae6ac55577dae585a1bb0794e7b38571245b9470d2c39dba5ae0edf7bb504ac825e759e6b994";

#[test]
fn download_song() -> anyhow::Result<()> {

    let agent = ureq::builder()
        .middleware(AuthMiddleware { user_agent: "Rizzle Test".to_string(), cookies: vec![MiddlewareCookie::new("arl", ARL_COOKIE)] })
        .build();

    // let song_quality = 1;
    
    // let result = agent.get("https://www.deezer.com/de/track/493679342").call()?.into_string()?;
    
    let song_query = r#"{"sng_ids":["619949882"]}"#;

    let result = agent.post("https://www.deezer.com/ajax/gw-light.php")
        .query("method", "song.getListData")
        .query("input", "3")
        .query("api_version", "1.0")
        .query("api_token", "MaERfUjJO_BYO~YtidECCmjLpRYlsgmb")
        .query("cid", "382490385") // Math.floor(1000000000 * Math.random())
        .set("Content-Length", &song_query.len().to_string())
        .set("x-deezer-user", "1578041862")
        .send_string(song_query)?
        .into_string()?;

    // let result: Value = agent.get(&format!("{DEEZER_API}/search/track?q=miraie")).call()?.into_json()?;

    println!("{result}");

    Ok(())

}

