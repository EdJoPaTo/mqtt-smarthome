#![forbid(unsafe_code)]

mod history_entry;
pub mod payload;
mod watcher;

use core::time::Duration;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

pub use history_entry::HistoryEntry;
use rumqttc::{AsyncClient, EventLoop, LastWill, MqttOptions, QoS};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::Receiver;
use tokio::sync::RwLock;
use tokio::task;
use tokio::time::sleep;
use watcher::Watcher;

#[derive(Clone)]
pub struct MqttSmarthome {
    client: AsyncClient,
    history: Arc<RwLock<HashMap<String, HistoryEntry>>>,
    last_will_retain: bool,
    last_will_topic: String,
    subscribed: Arc<RwLock<HashSet<String>>>,
    watchers: Arc<RwLock<Vec<Watcher>>>,
}

impl MqttSmarthome {
    #[must_use]
    pub fn new(base_topic: &str, host: &str, port: u16, last_will_retain: bool) -> Self {
        let last_will_topic = format!("{base_topic}/connected");

        let mut mqttoptions = MqttOptions::new(base_topic, host, port);
        mqttoptions.set_last_will(LastWill::new(
            &last_will_topic,
            "0",
            QoS::AtLeastOnce,
            last_will_retain,
        ));

        let (client, eventloop) = AsyncClient::new(mqttoptions, 100);

        let smarthome = Self {
            client,
            history: Arc::new(RwLock::new(HashMap::new())),
            last_will_retain,
            last_will_topic,
            subscribed: Arc::new(RwLock::new(HashSet::new())),
            watchers: Arc::new(RwLock::new(Vec::new())),
        };

        task::spawn({
            let smarthome = smarthome.clone();
            async move {
                handle_eventloop(&smarthome, eventloop).await;
            }
        });

        smarthome
    }

    #[allow(clippy::missing_errors_doc)]
    pub async fn disconnect(&self) -> Result<(), rumqttc::ClientError> {
        self.client.disconnect().await
    }

    pub async fn subscribe_and_watch(
        &self,
        topic: &str,
        allow_retained: bool,
    ) -> Receiver<watcher::ChannelPayload> {
        self.subscribe(topic).await;
        self.watch(topic, allow_retained).await
    }

    pub async fn subscribe(&self, topic: &str) {
        let is_new = self.subscribed.write().await.insert(topic.to_owned());
        if is_new {
            self.client
                .subscribe(topic, QoS::AtLeastOnce)
                .await
                .expect("failed to subscribe to MQTT");
        }
    }

    pub async fn watch(
        &self,
        topic: &str,
        allow_retained: bool,
    ) -> Receiver<watcher::ChannelPayload> {
        let (watcher, receiver) = Watcher::new(topic, allow_retained);
        self.watchers.write().await.push(watcher);
        receiver
    }

    pub async fn last(&self, topic: &str) -> Option<HistoryEntry> {
        self.history.read().await.get(topic).cloned()
    }

    pub async fn publish<P>(&self, topic: &str, payload: P, retain: bool)
    where
        P: ToString + Send,
    {
        let payload = payload.to_string();
        self.client
            .publish(topic, QoS::AtLeastOnce, retain, payload.clone())
            .await
            .expect("failed to publish to MQTT");

        self.history
            .write()
            .await
            .insert(topic.to_owned(), HistoryEntry::new(payload));
    }
}

async fn handle_eventloop(smarthome: &MqttSmarthome, mut eventloop: EventLoop) {
    loop {
        match eventloop.poll().await {
            Ok(rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(p))) => {
                println!("MQTT connected {p:?}");

                let smarthome = smarthome.clone();
                task::spawn(async move {
                    for topic in smarthome.subscribed.read().await.iter() {
                        smarthome
                            .client
                            .subscribe(topic, QoS::AtLeastOnce)
                            .await
                            .expect("failed to subscribe after reconnect");
                    }

                    smarthome
                        .client
                        .publish(
                            &smarthome.last_will_topic,
                            QoS::AtLeastOnce,
                            smarthome.last_will_retain,
                            "2",
                        )
                        .await
                        .expect("failed to publish connected");
                    println!("MQTT connection fully initialized");
                });
            }
            Ok(rumqttc::Event::Incoming(rumqttc::Incoming::Publish(publish))) => {
                if publish.dup {
                    continue;
                }

                if let Ok(payload) = String::from_utf8(publish.payload.to_vec()) {
                    smarthome
                        .history
                        .write()
                        .await
                        .insert(publish.topic.clone(), HistoryEntry::new(payload.clone()));

                    let senders = smarthome
                        .watchers
                        .read()
                        .await
                        .iter()
                        .filter_map(|o| o.matching_sender(&publish.topic, publish.retain))
                        .collect::<Vec<_>>();
                    for sender in senders {
                        match sender.try_send((publish.topic.clone(), payload.clone())) {
                            Ok(_) => {}
                            Err(TrySendError::Closed((topic, _))) => {
                                panic!("MQTT watcher receiver closed. Topic: {topic}");
                            }
                            Err(TrySendError::Full((topic, _))) => {
                                eprintln!("MQTT watcher receiver buffer is full. Topic: {topic}");
                            }
                        }
                    }
                }
            }
            Ok(rumqttc::Event::Outgoing(rumqttc::Outgoing::Disconnect)) => {
                println!("MQTT Disconnect happening...");
                break;
            }
            Ok(_) => {}
            Err(err) => {
                println!("MQTT Connection Error: {err}");
                sleep(Duration::from_secs(1)).await;
            }
        };
    }
}
