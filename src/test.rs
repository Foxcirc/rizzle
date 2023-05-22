
use std::fs;

use crate::{Credentials, Session};

#[test]
fn rizzle_test() {

    let cred_str = fs::read_to_string("../credentials").expect("Cannot read credentials");
    let cred_raw: Vec<&str> = cred_str.lines().collect();

    let cred = Credentials {
        sid: cred_raw[0].to_string(),
        arl: cred_raw[1].to_string(),
        api_token: cred_raw[2].to_string(),
    };
    
    let mut session = Session::new(cred);

    let result = session.search("Miraie").expect("Cannot search deezer");

    let artist = &result.artists[0];
    let artist_details = session.details(artist).expect("Cannot get artist details");
    assert!(artist.name == "Miraie", "Top artist not Miraie");
    assert!(artist_details.top_tracks.len() > 0, "Top-tracks empty");

    let album = &result.albums[0];
    let _album_details = session.details(album).expect("Cannot get album details");

    let new_token = session.end();

    assert!(&new_token == &cred_raw[2], "Token changed!")

}

