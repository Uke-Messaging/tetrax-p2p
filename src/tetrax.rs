use libp2p::{
    gossipsub::{
        self, error::PublishError, Gossipsub, GossipsubEvent, GossipsubMessage, IdentTopic,
        MessageId,
    },
    identity::Keypair,
    mdns,
    swarm::NetworkBehaviour,
    PeerId, Swarm,
};
use log::info;
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    error::Error,
    fs,
    hash::{Hash, Hasher},
    time::Duration,
};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "TetraxEvent")]
pub struct TetraxBehavior {
    pub gossipsub: Gossipsub,
    pub mdns: mdns::async_io::Behaviour,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum TetraxEvent {
    Gossipsub(GossipsubEvent),
    Mdns(mdns::Event),
}

impl From<mdns::Event> for TetraxEvent {
    fn from(v: mdns::Event) -> Self {
        Self::Mdns(v)
    }
}

impl From<GossipsubEvent> for TetraxEvent {
    fn from(v: GossipsubEvent) -> Self {
        Self::Gossipsub(v)
    }
}

// TODO add multiple topics frfr, then add them as needed when creating the network
pub struct Tetrax {
    pub topic: String,
    pub local_key: Keypair,
}

pub type Shares = Vec<Vec<u8>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub domain: String,
    pub shares: Shares,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SharesType {
    Complete,
    Partial,
}

impl Tetrax {
    /// Creates all necessary folders / files needed.
    pub fn new(local_key: Keypair, topic: String) -> Result<Self, std::io::Error> {
        let local_peer_id = PeerId::from(local_key.public());
        // Init struct here, ready for generating etc?
        fs::create_dir(&format!("/tmp/{}", local_peer_id)).expect("Unable to create node path");
        fs::write(&format!("/tmp/{}/shards.json", local_peer_id), "{}")?;
        info!("NEW NODE PATH CONFIGURED AT /tmp/{}", local_peer_id);
        Ok(Tetrax { local_key, topic })
    }

    pub async fn generate_swarm(&self) -> Result<Swarm<TetraxBehavior>, Box<dyn Error>> {
        let transport = libp2p::development_transport(self.local_key.clone()).await?;
        let message_id_fn = |message: &GossipsubMessage| {
            let mut s = DefaultHasher::new();
            // change message id maybe, unsure
            message.data.hash(&mut s);
            MessageId::from(s.finish().to_string())
        };

        // Configure Gossipsub behavior
        let gossipsub_config = gossipsub::GossipsubConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
            .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
            .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
            .build()?;

        let mut gossipsub: Gossipsub = Gossipsub::new(
            gossipsub::MessageAuthenticity::Signed(self.local_key.clone()),
            gossipsub_config,
        )?;
        let topic: IdentTopic = gossipsub::Topic::new(self.topic.clone());
        gossipsub.subscribe(&topic)?;
        let swarm: Swarm<TetraxBehavior> = {
            let mdns = mdns::async_io::Behaviour::new(mdns::Config::default())?;
            let behavior = TetraxBehavior { gossipsub, mdns };
            Swarm::with_async_std_executor(
                transport,
                behavior,
                PeerId::from_public_key(&self.local_key.public()),
            )
        };
        Ok(swarm)
    }

    /// Reads from local file
    pub fn read_local_shards(&self) -> Result<Vec<User>, std::io::Error> {
        let local_peer_id = PeerId::from(self.local_key.public());
        let bytes = fs::read(&format!("/tmp/{}", local_peer_id))?;
        let parsed_string = &String::from_utf8(bytes).expect("Unable to parse string");
        let user: Vec<User> = serde_json::from_str(parsed_string)?;
        Ok(user)
    }

    /// Write shards to the local file
    fn write_new_user(&self, user: User) -> Result<(), std::io::Error> {
        let local_peer_id = PeerId::from(self.local_key.public());
        let mut current_user = self.read_local_shards()?;
        current_user.push(user);
        let content = serde_json::to_string(&current_user)?;
        fs::write(&format!("/tmp/{}/shards.json", local_peer_id), content)?;
        Ok(())
    }

    /// Gossips shards to all peers
    fn publish_new_user(
        &self,
        swarm: &mut Swarm<TetraxBehavior>,
        user: User,
    ) -> Result<MessageId, PublishError> {
        let topic: IdentTopic = gossipsub::Topic::new(self.topic.clone());
        swarm.behaviour_mut().gossipsub.publish(
            topic,
            serde_json::to_string(&user)
                .expect("Correctly parsed")
                .as_bytes(),
        )
    }
}
