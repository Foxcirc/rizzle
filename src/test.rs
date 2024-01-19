
use crate::{UserInfo, Session};

use std::io::Read;
use futures_lite::future::block_on;
use serde_derive::Deserialize;

#[derive(Default, Deserialize)]
struct Config {
    pub(crate) info: UserInfo,
}

#[test]
fn rizzle_test() {

    block_on(async {

        let config_str = std::fs::read_to_string("Dizzle.toml").unwrap();
        let config: Config = toml::from_str(&config_str).unwrap();
        let info = config.info;
    
        let mut session = Session::new(info).await.expect("create new Session");
        println!("created session");

        let result = session.search("troye sivan - easy").await.expect("search deezer");
        println!("found artist miraie ({} tracks)", result.tracks.len());
        println!("--- results ---\n{:#?}", result);

        // let artist = &result.artists[0];
        // let artist_details = session.details(artist).await.expect("get artist details");
        // assert!(artist.name == "Miraie", "Top artist not Miraie");
        // assert!(artist_details.top_tracks.len() > 0, "Top-tracks empty");
        // println!("got artist details");

        // let album = &result.albums[0];
        // let _album_details = session.details(album).await.expect("get album details");
        // println!("got album details");

        // let's download a song!

        let track = &result.tracks[1];
        println!("downloading {}", track.name);
        let mut handle = session.stream_raw(track).await.expect("stream raw");

        // pulseaudio player

        let spec = libpulse_binding::sample::Spec {
            format: libpulse_binding::sample::Format::S16NE,
            channels: 2,
            rate: 44100,
        };

        assert!(spec.is_valid());

        let audio = libpulse_simple_binding::Simple::new(
            None,                // Use the default server
            "dizzle",            // Our application’s name
            libpulse_binding::stream::Direction::Playback, // We want a playback stream
            None,                // Use the default device
            "Music",             // Description of our stream
            &spec,               // Our sample format
            None,                // Use default channel map
            None                 // Use default buffering attributes
        ).unwrap();

        let mut buff = [0; 1024];
        while handle.read_exact(&mut buff).is_ok() {
            audio.write(&buff).unwrap();
        }

        audio.drain().unwrap();

        // let mut file = std::fs::File::create("track.raw").unwrap();
        
        // for packet in handle.map(|it| it.unwrap()).flatten() {
        //     file.write(&packet.to_ne_bytes()).unwrap();
        // }

    })

}

