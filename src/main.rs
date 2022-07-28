use chrono::{prelude::*, DateTime};
use libp2p::{
    core::upgrade,
    floodsub::{Floodsub, FloodsubEvent, Topic},
    futures::StreamExt,
    identity,
    mdns::{Mdns, MdnsEvent},
    mplex, noise,
    swarm::{NetworkBehaviourEventProcess, Swarm, SwarmBuilder},
    tcp::TokioTcpTransport,
    NetworkBehaviour, PeerId, Transport,
};
use once_cell::sync::OnceCell;
use std::error::Error;
use std::{borrow::BorrowMut, env};
use tokio::io::AsyncBufReadExt;

mod message;
use message::{Message, Person};

const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S %z";

static TOPIC: OnceCell<Topic> = OnceCell::new();

#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
struct ChatBehaviour {
    floodsub: Floodsub,
    mdns: Mdns,
    #[behaviour(ignore)]
    person: Person,
}

impl NetworkBehaviourEventProcess<MdnsEvent> for ChatBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(list) => {
                for (peer, _) in list {
                    self.floodsub.add_node_to_partial_view(peer);
                }
            }
            MdnsEvent::Expired(list) => {
                for (peer, _) in list {
                    if !self.mdns.has_node(&peer) {
                        self.floodsub.remove_node_from_partial_view(&peer);
                    }
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess<FloodsubEvent> for ChatBehaviour {
    fn inject_event(&mut self, event: FloodsubEvent) {
        match event {
            FloodsubEvent::Message(msg) => {
                if let Ok(mut content) = serde_json::from_slice::<Message>(&msg.data) {
                    let utc = DateTime::parse_from_str(&content.datetime, DATETIME_FORMAT).unwrap();
                    let dt = utc.with_timezone(&Local::now().timezone());
                    content.borrow_mut().datetime = dt.format(DATETIME_FORMAT).to_string();
                    println!("{}", content);
                } else {
                    eprintln!("Received unknow content");
                    return;
                }
            }
            _ => {}
        }
    }
}

/// this function will get the topic from arguments and alias of user
fn get_info_from_args() -> Person {
    if let Some(t) = env::args().nth(1) {
        TOPIC.set(Topic::new(t)).unwrap();
    } else {
        TOPIC.set(Topic::new("public-chat")).unwrap();
    }

    let alias = if let Some(alias) = env::args().nth(2) {
        alias
    } else {
        "unknow".to_string()
    };

    Person { alias }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let current_person = get_info_from_args();

    let id_keys = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(id_keys.public());

    let noise_keys = noise::Keypair::<noise::X25519Spec>::new().into_authentic(&id_keys)?;

    let transport = TokioTcpTransport::default()
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(mplex::MplexConfig::new())
        .boxed();

    let mut swarm = {
        let mdns = Mdns::new(Default::default()).await?;
        let behaviour = ChatBehaviour {
            floodsub: Floodsub::new(peer_id.clone()),
            mdns,
            person: current_person,
        };

        SwarmBuilder::new(transport, behaviour, peer_id)
            .executor(Box::new(|fut| {
                tokio::spawn(fut);
            }))
            .build()
    };
    swarm.behaviour_mut().floodsub.subscribe(
        TOPIC
            .get()
            .expect("can not get TOPIC while subscribe TOPIC")
            .to_owned(),
    );

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();

    loop {
        tokio::select! {
            line = stdin.next_line() => {
                let msg = line.expect("can not get line").expect("can not read line from stdin");
                send_message(msg, &mut swarm);
            }
            _event = swarm.select_next_some() => { 
                continue;
            }
        }
    }
}

fn send_message(content: String, swarm: &mut Swarm<ChatBehaviour>) {
    let content = content.trim();
    if content.is_empty() {
        return;
    }
    let topic = TOPIC
        .get()
        .expect("can not get TOPIC while send message")
        .to_owned();
    let now_utc = Utc::now();
    let datetime = now_utc.format(DATETIME_FORMAT).to_string();

    let message = Message {
        alias: swarm.behaviour().person.clone(),
        content: content.to_string(),
        datetime,
    };
    let json = serde_json::to_string(&message).unwrap();
    swarm
        .behaviour_mut()
        .floodsub
        .publish(topic, json.as_bytes());
}
