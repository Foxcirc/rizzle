
use crate::{UserInfo, Session};

use serde_derive::Deserialize;

#[derive(Default, Deserialize)]
struct Config {
    pub(crate) info: UserInfo,
}

#[test]
fn rizzle_test() {

    let config_str = std::fs::read_to_string("Dizzle.toml").unwrap();
    let config: Config = toml::from_str(&config_str).unwrap();
    let info = config.info;
    
    let mut session = Session::new(info).expect("Cannot create new Session");

    let result = session.search("Miraie").expect("Cannot search deezer");

    let artist = &result.artists[0];
    let artist_details = session.details(artist).expect("Cannot get artist details");
    assert!(artist.name == "Miraie", "Top artist not Miraie");
    assert!(artist_details.top_tracks.len() > 0, "Top-tracks empty");

    let album = &result.albums[0];
    let _album_details = session.details(album).expect("Cannot get album details");

}

