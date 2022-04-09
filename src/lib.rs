#![forbid(unsafe_code)]

mod history_entry;
pub mod payload;
mod watcher;

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

pub use history_entry::HistoryEntry;
use rumqttc::{AsyncClient, EventLoop, LastWill, MqttOptions, QoS};
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use tokio::time::sleep;
use watcher::Watcher;

#[derive(Clone)]
pub struct MqttSmarthome {
    client: AsyncClient,
    history: Arc<Mutex<HashMap<String, HistoryEntry>>>,
    last_will_retain: bool,
    last_will_topic: String,
    subscribed: Arc<Mutex<HashSet<String>>>,
    watchers: Arc<Mutex<Vec<Watcher>>>,
}

impl MqttSmarthome {
    #[must_use]
    pub fn new(
        base_topic: &str,
        host: &str,
        port: u16,
        last_will_retain: bool,
    ) -> (Self, EventLoop) {
        let last_will_topic = format!("{}/connected", base_topic);

        let mut mqttoptions = MqttOptions::new(base_topic, host, port);
        mqttoptions.set_last_will(LastWill::new(
            &last_will_topic,
            "0",
            QoS::AtLeastOnce,
            last_will_retain,
        ));

        let (client, eventloop) = AsyncClient::new(mqttoptions, 100);

        (
            Self {
                client,
                history: Arc::new(Mutex::new(HashMap::new())),
                last_will_retain,
                last_will_topic,
                subscribed: Arc::new(Mutex::new(HashSet::new())),
                watchers: Arc::new(Mutex::new(Vec::new())),
            },
            eventloop,
        )
    }

    pub async fn handle_eventloop(&self, mut eventloop: EventLoop) {
        loop {
            match eventloop.poll().await {
                Ok(rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(p))) => {
                    println!("MQTT connected {:?}", p);

                    if !p.session_present {
                        for topic in self.subscribed.lock().await.iter() {
                            self.client
                                .subscribe(topic, QoS::AtLeastOnce)
                                .await
                                .expect("failed to subscribe after reconnect");
                        }
                    }

                    self.client
                        .publish(
                            &self.last_will_topic,
                            QoS::AtLeastOnce,
                            self.last_will_retain,
                            "2",
                        )
                        .await
                        .expect("failed to publish connected");
                }
                Ok(rumqttc::Event::Incoming(rumqttc::Incoming::Publish(publish))) => {
                    if publish.dup {
                        continue;
                    }

                    if let Ok(payload) = String::from_utf8(publish.payload.to_vec()) {
                        self.history
                            .lock()
                            .await
                            .insert(publish.topic.clone(), HistoryEntry::new(payload.clone()));

                        for watcher in self.watchers.lock().await.iter() {
                            watcher
                                .notify(&publish.topic, publish.retain, &payload)
                                .await
                                .expect("failed to send to mqtt watcher");
                        }
                    }
                }
                Ok(rumqttc::Event::Outgoing(rumqttc::Outgoing::Disconnect)) => {
                    println!("MQTT Disconnect happening...");
                    break;
                }
                Ok(_) => {}
                Err(err) => {
                    println!("MQTT Connection Error: {}", err);
                    sleep(Duration::from_secs(1)).await;
                }
            };
        }
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
        let is_new = self.subscribed.lock().await.insert(topic.to_owned());
        if is_new {
            self.client
                .subscribe(topic, QoS::AtLeastOnce)
                .await
                .expect("failed to subscribe to mqtt");
        }
    }

    pub async fn watch(
        &self,
        topic: &str,
        allow_retained: bool,
    ) -> Receiver<watcher::ChannelPayload> {
        let (watcher, receiver) = Watcher::new(topic, allow_retained);
        self.watchers.lock().await.push(watcher);
        receiver
    }

    pub async fn last(&self, topic: &str) -> Option<HistoryEntry> {
        self.history.lock().await.get(topic).cloned()
    }

    pub async fn publish<P>(&self, topic: &str, payload: P, retain: bool)
    where
        P: ToString + Send,
    {
        let payload = payload.to_string();
        self.client
            .publish(topic, QoS::AtLeastOnce, retain, payload.clone())
            .await
            .expect("failed to publish to mqtt");

        self.history
            .lock()
            .await
            .insert(topic.to_owned(), HistoryEntry::new(payload));
    }
}
