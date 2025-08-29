use core::time::Duration;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use rumqttc::{AsyncClient, EventLoop, LastWill, MqttOptions, QoS};
use tokio::sync::RwLock;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::task;
use tokio::time::sleep;

pub use self::history_entry::HistoryEntry;
use self::watcher::Watcher;

mod history_entry;
pub mod payload;
mod subscriptions;
mod watcher;

#[derive(Clone)]
pub struct MqttSmarthome {
    client: AsyncClient,
    history: Arc<RwLock<HashMap<String, HistoryEntry>>>,
    last_received: Arc<RwLock<Option<SystemTime>>>,
    last_will_retain: bool,
    last_will_topic: String,
    subscribed: Arc<RwLock<subscriptions::Subscriptions>>,
    #[expect(clippy::type_complexity)]
    watchers: Arc<RwLock<Vec<Watcher<Sender<(String, String)>>>>>,
}

impl MqttSmarthome {
    #[must_use]
    pub fn new(base_topic: &str, host: &str, port: u16, last_will_retain: bool) -> Self {
        let last_will_topic = format!("{base_topic}/connected");
        let mqttoptions = MqttOptions::new(base_topic, host, port);
        Self::new_options(last_will_topic, last_will_retain, mqttoptions)
    }

    #[must_use]
    pub fn new_options(
        last_will_topic: String,
        last_will_retain: bool,
        mut mqttoptions: MqttOptions,
    ) -> Self {
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
            last_received: Arc::new(RwLock::new(None)),
            subscribed: Arc::new(RwLock::new(subscriptions::Subscriptions::new())),
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

    /// Disconnect from the MQTT broker.
    #[expect(clippy::missing_errors_doc)]
    pub async fn disconnect(&self) -> Result<(), rumqttc::ClientError> {
        self.client.disconnect().await
    }

    /// Subscribe to MQTT packages where topics match the given `filter`.
    /// # Panics
    /// Panics when the MQTT eventloop is gone.
    pub async fn subscribe(&self, filter: &str) {
        let is_new = self.subscribed.write().await.add(filter);
        if is_new {
            self.client
                .subscribe(filter, QoS::AtLeastOnce)
                .await
                .expect("failed to subscribe to MQTT");
        }
    }

    /// Create a channel that receives messages for topics that match the given `filter`.
    ///
    /// Also, [subscribe](Self::subscribe)s to the given `filter` on the broker.
    pub async fn subscribe_channel(
        &self,
        filter: &str,
        allow_retained: bool,
    ) -> Receiver<(String, String)> {
        let (sender, receiver) = channel(25);
        let watcher = Watcher::new(filter, allow_retained, sender);
        self.watchers.write().await.push(watcher);
        self.subscribe(filter).await;
        receiver
    }

    #[must_use]
    pub async fn since_last_received(&self) -> Option<Duration> {
        self.last_received
            .read()
            .await
            .and_then(|last_received| last_received.elapsed().ok())
    }

    /// Return the last `HistoryEntry` of the given `topic`.
    pub async fn last(&self, topic: &str) -> Option<HistoryEntry> {
        self.history.read().await.get(topic).cloned()
    }

    /// Shortcut for `.last(topic).await.map(|entry| entry.as_boolean())` without clone.
    pub async fn last_as_bool(&self, topic: &str) -> Option<bool> {
        self.history
            .read()
            .await
            .get(topic)
            .map(HistoryEntry::as_boolean)
    }

    /// Shortcut for `.last(topic).await.and_then(|entry| entry.as_float())` without clone.
    pub async fn last_as_float(&self, topic: &str) -> Option<f32> {
        self.history
            .read()
            .await
            .get(topic)
            .and_then(HistoryEntry::as_float)
    }

    /// Shortcut for `.last(topic).await.is_some_and(|entry| entry.as_boolean())` without clone.
    pub async fn last_is_true(&self, topic: &str) -> bool {
        self.history
            .read()
            .await
            .get(topic)
            .is_some_and(HistoryEntry::as_boolean)
    }

    /// Publish a `payload` to a MQTT `topic`.
    /// # Panics
    /// Panics when the MQTT eventloop is gone.
    pub async fn publish<T, P>(&self, topic: T, payload: P, retain: bool)
    where
        T: Into<String>,
        P: ToString + Send,
    {
        let topic = topic.into();
        let payload = payload.to_string();
        self.client
            .publish(topic.clone(), QoS::AtLeastOnce, retain, payload.clone())
            .await
            .expect("failed to publish to MQTT");
        let time = SystemTime::now();
        self.history
            .write()
            .await
            .insert(topic, HistoryEntry::new(time, payload));
    }
}

async fn handle_eventloop(smarthome: &MqttSmarthome, mut eventloop: EventLoop) {
    loop {
        match eventloop.poll().await {
            Ok(rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(packet))) => {
                println!("MQTT connected {packet:?}");

                let smarthome = smarthome.clone();
                task::spawn(async move {
                    let topics = smarthome.subscribed.read().await.0.clone();
                    #[expect(clippy::iter_over_hash_type)]
                    for topic in topics {
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
            Ok(rumqttc::Event::Incoming(rumqttc::Incoming::Publish(publish))) if !publish.dup => {
                let time = SystemTime::now();
                if let Ok(payload) = String::from_utf8(publish.payload.into()) {
                    *smarthome.last_received.write().await = Some(time);
                    smarthome.history.write().await.insert(
                        publish.topic.clone(),
                        HistoryEntry::new(time, payload.clone()),
                    );
                    smarthome
                        .watchers
                        .read()
                        .await
                        .iter()
                        .filter_map(|watcher| watcher.matching(&publish.topic, publish.retain))
                        .for_each(|sender| {
                            match sender.try_send((publish.topic.clone(), payload.clone())) {
                                Ok(()) => {}
                                Err(TrySendError::Closed((topic, _))) => {
                                    panic!("MQTT watch receiver closed. Topic: {topic}");
                                }
                                Err(TrySendError::Full((topic, _))) => {
                                    eprintln!("MQTT watch receiver buffer is full. Topic: {topic}");
                                }
                            }
                        });
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
        }
    }
}
