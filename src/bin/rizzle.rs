
/*
* Rizzler is an alternative deezer client.
*/

use rizzle::*;

const ARL_COOKIE: &str = "d13c0a4ecc37fe7f0986cc97cb16cc60e0dd96195b7193fac903a18c3f23e4e8355167a9367a04371eb446b33520f578c65b9bacb33d3fc1e755ae6ac55577dae585a1bb0794e7b38571245b9470d2c39dba5ae0edf7bb504ac825e759e6b994";
const SID_COOKIE: &str = "fr9f76aaa5c6d50a2b94d73e6bdbb64c6b243565";

fn main() -> anyhow::Result<()> {
    
    let info = Info {
        user_agent: "Rizzle",
        arl: ARL_COOKIE,
        sid: SID_COOKIE,
    };

    let session = Session::new(info)?;

    // let result = session.search(Search::Playlist, "Heiakim")?;
    // let first = result[0];
    // let song = first[0];
    // 
    // let stream = session.steam(song)?;
    // 
    // let bytes: Vec<u8> = stream.collect();

    Ok(())

}

