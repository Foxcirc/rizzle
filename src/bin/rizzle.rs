
/*
* Rizzler is an alternative deezer client.
*/

use rizzle::*;

fn main() -> anyhow::Result<()> {
    
    let mut auth = DeezerAuth {
        sid: "frcb27617550d648c305cbc0628bd86a2fd81cc1".to_string(),
        arl: "d13c0a4ecc37fe7f0986cc97cb16cc60e0dd96195b7193fac903a18c3f23e4e8355167a9367a04371eb446b33520f578c65b9bacb33d3fc1e755ae6ac55577dae585a1bb0794e7b38571245b9470d2c39dba5ae0edf7bb504ac825e759e6b994".to_string(),
    };

    let session = Session::new(auth)?;

    let result = session.search("Heiakim")?;

    println!("{:?}", result);

    // let stream = session.steam(song)?;
    // 
    // let bytes: Vec<u8> = stream.collect();

    auth = session.end();
    
    Ok(())

}

