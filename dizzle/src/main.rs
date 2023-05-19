
/*
* Rizzler is an alternative deezer client.
*/

fn main() -> anyhow::Result<()> {
    
    let credentials = rizzle::Credentials {
        sid: "frcb27617550d648c305cbc0628bd86a2fd81cc1".to_string(),
        arl: "d13c0a4ecc37fe7f0986cc97cb16cc60e0dd96195b7193fac903a18c3f23e4e8355167a9367a04371eb446b33520f578c65b9bacb33d3fc1e755ae6ac55577dae585a1bb0794e7b38571245b9470d2c39dba5ae0edf7bb504ac825e759e6b994".to_string(),
    };

    let session = rizzle::Session::new(credentials)?;

    let result = session.search("just the two of us")?;

    println!("Streaming {:?}", result.tracks[0]);

    let stream = session.stream(&result.tracks[0])?;

    let pcm = alsa::PCM::new("default", alsa::Direction::Playback, false)?;

    let params = alsa::pcm::HwParams::any(&pcm)?;
    params.set_channels(2)?;
    params.set_rate(44100, alsa::ValueOr::Nearest)?;
    params.set_format(alsa::pcm::Format::s16())?;
    params.set_access(alsa::pcm::Access::RWInterleaved)?;

    pcm.hw_params(&params)?;
    let alsa_io = pcm.io_i16()?;

    let mut decoder = minimp3::Decoder::new(stream);

    let mut last_frame = None;

    loop {
        match decoder.next_frame() {
            Ok(frame) => {
                alsa_io.writei(&frame.data)?;
                last_frame = Some(frame);
            },
            Err(minimp3::Error::Eof) => break,
            Err(err) => Err(err)?,
        }
    }

    if let Some(frame) = last_frame {
        println!("data len: {}, channels: {}, bitrate: {}kbps, sample rate: {}", frame.data.len(), frame.channels, frame.bitrate, frame.sample_rate);
    }

    pcm.drain()?;

    _ = session.end();
    
    Ok(())

}

