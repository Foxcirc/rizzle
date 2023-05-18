
use serde_json::Value;
use crate::util::OptionToResult;

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

