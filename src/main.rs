use async_std::io;
use futures::{
    prelude::{stream::StreamExt, *},
    select,
};
use libp2p::{
    gossipsub::{Gossipsub, GossipsubEvent},
    identity, mdns,
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId, Swarm,
};
use log::{info, warn};
use shamir::SecretData;
use std::{error::Error, fs};

use ::tetrax::tetrax::{TetraxEvent, Tetrax};

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    // Create a random PeerId
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    info!("WELCOME TO TETRAX.");
    info!("'I discovered a chance for redemption.'");
    warn!("HIGHLY EXPERIMENTAL!!!");
    info!("Local peer id:for  {local_peer_id:?}");

    let tetrax = Tetrax::new(local_key, "identity".to_string())?;
    let mut swarm = tetrax.generate_swarm().await?;
    
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {address:?}");
            }
            SwarmEvent::Behaviour(TetraxEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, _multiaddr) in list {
                    println!("mDNS discovered a new peer: {peer_id}");
                    swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                }
            }
            SwarmEvent::Behaviour(TetraxEvent::Mdns(mdns::Event::Expired(list))) => {
                for (peer_id, _multiaddr) in list {
                    println!("mDNS discover peer has expired: {peer_id}");
                    swarm
                        .behaviour_mut()
                        .gossipsub
                        .remove_explicit_peer(&peer_id);
                }
            }
            SwarmEvent::Behaviour(TetraxEvent::Gossipsub(GossipsubEvent::Message {
                propagation_source: peer_id,
                message_id: id,
                message,
            })) => println!(
                "Got message: '{}' with id: {id} from peer: {peer_id}",
                String::from_utf8_lossy(&message.data),
            ),
            _ => {}
        }
    }

    Ok(())
}
