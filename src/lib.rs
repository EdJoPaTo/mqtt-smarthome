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
    subscriptions: Arc<RwLock<subscriptions::Subscriptions>>,
    #[expect(clippy::type_complexity)]
    watchers: Arc<RwLock<Vec<Watcher<Sender<(String, String)>>>>>,
}

impl MqttSmarthome {
    /// Connect to the MQTT Broker.
    /// # Panics
    /// When the initial connection fails.
    #[must_use]
    pub async fn new(
        base_topic: &str,
        host: &str,
        port: u16,
        last_will_retain: bool,
    ) -> (Self, EventLoop) {
        let last_will_topic = format!("{base_topic}/online");
        let mqttoptions = MqttOptions::new(base_topic, host, port);
        Self::new_options(last_will_topic, last_will_retain, mqttoptions).await
    }

    /// Connect to the MQTT Broker.
    /// # Panics
    /// When the initial connection fails.
    #[must_use]
    pub async fn new_options(
        last_will_topic: String,
        last_will_retain: bool,
        mut mqttoptions: MqttOptions,
    ) -> (Self, EventLoop) {
        mqttoptions.set_last_will(LastWill::new(
            &last_will_topic,
            "false",
            QoS::AtLeastOnce,
            last_will_retain,
        ));

        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 100);

        let smarthome = Self {
            client,
            history: Arc::new(RwLock::new(HashMap::new())),
            last_will_retain,
            last_will_topic,
            last_received: Arc::new(RwLock::new(None)),
            subscriptions: Arc::new(RwLock::new(subscriptions::Subscriptions::new())),
            watchers: Arc::new(RwLock::new(Vec::new())),
        };

        loop {
            match eventloop.poll().await {
                Ok(rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(packet))) => {
                    eprintln!("Initial MQTT connection successful {packet:?}");
                    break;
                }
                Ok(event) => eprintln!("Unexpected event on expected initial ConnAck: {event:?}"),
                Err(err) => panic!("Initial MQTT connection error: {err}"),
            }
        }

        (smarthome, eventloop)
    }

    /// Disconnect from the MQTT broker.
    #[expect(clippy::missing_errors_doc)]
    pub async fn disconnect(&self) -> Result<(), rumqttc::ClientError> {
        self.client.disconnect().await
    }

    /// Add the `filter` to the subscriptions which will be subscribed on the Broker on [`start_eventloop`](Self::start_eventloop).
    ///
    /// Should be called on startup before [`start_eventloop`](Self::start_eventloop).
    /// After that it will print a warning about doing that on startup instead and manually subscribes to the topic (so it will still work but less optimal).
    /// # Panics
    /// Panics when the MQTT eventloop is gone.
    pub async fn subscribe(&self, filter: &str) {
        let already_received_something = self.since_last_received().await.is_some();
        if already_received_something {
            eprintln!(
                "MQTT subscribe called after start_subscriptions. Consider defining the subscription on startup."
            );
        }
        let is_new = self.subscriptions.write().await.add(filter);
        if is_new && already_received_something {
            self.client
                .subscribe(filter, QoS::AtLeastOnce)
                .await
                .expect("should subscribe to MQTT");
        }
    }

    /// Create a channel that receives messages for topics that match the given `filter`.
    ///
    /// Also, [subscribe](Self::subscribe)s to the given `filter` on the broker.
    ///
    /// `buffer` should be big enough for the incoming messages on a bulk.
    /// When retained are allowed there should be enough space to get all retained messages into the buffer on startup.
    /// Must be at least 1.
    #[must_use]
    pub async fn subscribe_channel(
        &self,
        filter: &str,
        allow_retained: bool,
        buffer: usize,
    ) -> Receiver<(String, String)> {
        let (sender, receiver) = channel(buffer);
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
    #[must_use]
    pub async fn last(&self, topic: &str) -> Option<HistoryEntry> {
        self.history.read().await.get(topic).cloned()
    }

    /// Shortcut for `.last(topic).await.map(|entry| entry.as_boolean())` without clone.
    #[must_use]
    pub async fn last_as_bool(&self, topic: &str) -> Option<bool> {
        self.history
            .read()
            .await
            .get(topic)
            .map(HistoryEntry::as_boolean)
    }

    /// Shortcut for `.last(topic).await.and_then(|entry| entry.as_float())` without clone.
    #[must_use]
    pub async fn last_as_float(&self, topic: &str) -> Option<f32> {
        self.history
            .read()
            .await
            .get(topic)
            .and_then(HistoryEntry::as_float)
    }

    /// Shortcut for `.last(topic).await.is_some_and(|entry| entry.as_boolean())` without clone.
    #[must_use]
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
            .expect("should publish to MQTT");
        let time = SystemTime::now();
        self.history
            .write()
            .await
            .insert(topic, HistoryEntry::new(time, payload));
    }

    /// Handle the MQTT eventloop.
    ///
    /// This method will block and only end when a disconnect is happening.
    /// Either run as the main loop or in its own task.
    /// Should be started after startup and [`subscribe`s](Self::subscribe) are all done.
    pub async fn start_eventloop(&self, eventloop: EventLoop) {
        handle_eventloop(self, eventloop).await;
    }
}

fn on_connect(smarthome: MqttSmarthome) {
    task::spawn(async move {
        let topics = smarthome.subscriptions.read().await.0.clone();
        if !topics.is_empty() {
            eprintln!("MQTT subscribe to {} topics…", topics.len());
        }
        #[expect(clippy::iter_over_hash_type)]
        for topic in topics {
            smarthome
                .client
                .subscribe(topic, QoS::AtLeastOnce)
                .await
                .expect("MQTT should subscribe topics");
        }

        smarthome
            .client
            .publish(
                &smarthome.last_will_topic,
                QoS::AtLeastOnce,
                smarthome.last_will_retain,
                "true",
            )
            .await
            .expect("MQTT should publish connection status");
        eprintln!("MQTT connection fully initialized");
    });
}

async fn handle_eventloop(smarthome: &MqttSmarthome, mut eventloop: EventLoop) {
    on_connect(smarthome.clone());
    loop {
        match eventloop.poll().await {
            Ok(rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(packet))) => {
                eprintln!("MQTT connected {packet:?}");
                on_connect(smarthome.clone());
            }
            Ok(rumqttc::Event::Incoming(rumqttc::Incoming::Publish(publish))) if !publish.dup => {
                let time = SystemTime::now();
                if let Ok(payload) = String::from_utf8(publish.payload.into()) {
                    *smarthome.last_received.write().await = Some(time);
                    smarthome.history.write().await.insert(
                        publish.topic.clone(),
                        HistoryEntry::new(time, payload.clone()),
                    );
                    for watcher in smarthome.watchers.read().await.iter() {
                        if let Some(sender) = watcher.matching(&publish.topic, publish.retain) {
                            match sender.try_send((publish.topic.clone(), payload.clone())) {
                                Ok(()) => {}
                                Err(TrySendError::Closed((topic, _))) => panic!(
                                    "MQTT watch receiver closed. Filter: {} Topic: {topic}",
                                    watcher.filter()
                                ),
                                Err(TrySendError::Full((topic, _))) => eprintln!(
                                    "MQTT watch receiver buffer is full. Filter: {} Topic: {topic}",
                                    watcher.filter()
                                ),
                            }
                        }
                    }
                }
            }
            Ok(rumqttc::Event::Outgoing(rumqttc::Outgoing::Disconnect)) => {
                eprintln!("MQTT Disconnect happening...");
                break;
            }
            Ok(_) => {}
            Err(err) => {
                eprintln!("MQTT Connection Error: {err}");
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
