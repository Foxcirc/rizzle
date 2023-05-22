
/*
* Dizzle is an alternative deezer client.
*/

use std::fs;

fn main() -> anyhow::Result<()> {
    
    let args: Vec<String> = std::env::args().collect();

    let cred_str = fs::read_to_string("credentials")?;
    let cred_lines: Vec<&str> = cred_str.lines().take(3).collect();

    let credentials = rizzle::Credentials {
        sid: cred_lines[0].to_string(),
        arl: cred_lines[1].to_string(),
        api_token: cred_lines[2].to_string(),
    };

    let mut session = rizzle::Session::new(credentials);

    let result = session.search(&args[1])?;

    let track = &result.tracks[0];
    println!("Streaming {:?}", track.name);
    println!("Artist {:?}", track.artists[0].name);

    let details = session.details(&track.artists[0])?;
    println!(" -> Total Albums: {}", details.albums.len());
    println!(" -> First Album: {:?}", details.albums[0]);

    // let alb_songs = session.details(&details.albums[0])?;
    // println!(" -> Total Album Songs: {:?}", alb_songs.tracks.len());

    let stream = session.stream_raw(&track)?;

    let pcm = alsa::PCM::new("default", alsa::Direction::Playback, false)?;

    let params = alsa::pcm::HwParams::any(&pcm)?;
    params.set_channels(2)?;
    params.set_rate(44100, alsa::ValueOr::Nearest)?;
    params.set_format(alsa::pcm::Format::s16())?;
    params.set_access(alsa::pcm::Access::RWInterleaved)?;

    pcm.hw_params(&params)?;
    let alsa_io = pcm.io_checked::<i16>()?;

    let hwp = pcm.hw_params_current().unwrap();
    let swp = pcm.sw_params_current().unwrap();
    swp.set_start_threshold(hwp.get_buffer_size().unwrap()).unwrap();
    pcm.sw_params(&swp).unwrap();

    for packet in stream {
        alsa_io.writei(&packet?)?;
    }

    pcm.drain()?;

    let api_token = session.end();
    
    let mut content = fs::read_to_string("credentials")?;
    for (idx, line) in content.clone().lines().enumerate() {
        if idx == 3 {
            content = content.replace(line, &api_token);
        }
    }

    fs::write("credentials", content)?;
    
    Ok(())

}

