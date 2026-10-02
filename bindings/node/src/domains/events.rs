//! Events API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Events API
// ============================================================================

#[napi(object)]
pub struct CreateWebhookInput {
    /// Display name
    pub name: Option<String>,
    /// Target URL for POST requests
    pub url: String,
    /// Optional secret for HMAC signature
    pub secret: Option<String>,
    /// Event types to receive (empty or omitted = all events)
    pub event_types: Option<Vec<String>>,
}

#[napi(object)]
pub struct WebhookOutput {
    pub id: String,
    pub name: String,
    pub url: String,
    pub has_secret: bool,
    pub event_types: Vec<String>,
    pub active: bool,
    pub created_at: String,
}

impl From<stateset_embedded::Webhook> for WebhookOutput {
    fn from(w: stateset_embedded::Webhook) -> Self {
        Self {
            id: w.id.to_string(),
            name: w.name,
            url: w.url,
            has_secret: w.secret.is_some(),
            event_types: w.event_types,
            active: w.active,
            created_at: w.created_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Events {
    pub(crate) commerce: Handle,
}

#[napi]
impl Events {
    /// Subscribe to all commerce events.
    #[napi]
    pub async fn subscribe(&self) -> Result<CommerceEventSubscription> {
        let commerce = self.commerce.get()?;
        Ok(CommerceEventSubscription::new(commerce.subscribe_events(), None))
    }

    /// Subscribe to a subset of commerce events by event type.
    ///
    /// Event types must match `CommerceEvent::event_type()` values (snake_case),
    /// e.g. "order_created", "inventory_adjusted".
    #[napi]
    pub async fn subscribe_filtered(
        &self,
        event_types: Vec<String>,
    ) -> Result<CommerceEventSubscription> {
        let commerce = self.commerce.get()?;
        let allowed: HashSet<String> = event_types.into_iter().collect();
        let allowed = if allowed.is_empty() { None } else { Some(allowed) };
        Ok(CommerceEventSubscription::new(commerce.subscribe_events(), allowed))
    }

    /// List registered webhooks.
    #[napi]
    pub async fn list_webhooks(&self) -> Result<Vec<WebhookOutput>> {
        let commerce = self.commerce.get()?;
        let webhooks = commerce.list_webhooks();
        Ok(webhooks.into_iter().map(|w| w.into()).collect())
    }

    /// Register a webhook endpoint for event delivery.
    #[napi]
    pub async fn register_webhook(&self, input: CreateWebhookInput) -> Result<Option<String>> {
        let commerce = self.commerce.get()?;

        let name = input.name.unwrap_or_else(|| "Webhook".to_string());
        let mut webhook = stateset_embedded::Webhook::new(name, input.url);
        if let Some(secret) = input.secret {
            webhook = webhook.with_secret(secret);
        }
        if let Some(events) = input.event_types {
            if !events.is_empty() {
                webhook = webhook.with_events(events);
            }
        }

        Ok(Some(commerce.register_webhook(webhook).to_string()))
    }

    /// Unregister a webhook endpoint.
    #[napi]
    pub async fn unregister_webhook(&self, id: String) -> Result<bool> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;
        Ok(commerce.unregister_webhook(uuid))
    }
}

pub(crate) type EventCallback = ThreadsafeFunction<Option<serde_json::Value>, ErrorStrategy::Fatal>;

/// A stream of commerce events.
///
/// Delivery runs on the binding's runtime and hands each event to a JavaScript
/// callback through a threadsafe function that starts *unreferenced*, so a
/// subscription never keeps the process alive by itself. (Every napi async
/// method resolves its promise through a *referenced* threadsafe function,
/// which is why the previous `recv()` — a native promise that could stay
/// pending forever — pinned the event loop at exit.) The `recv()` and
/// async-iterator surface lives in `index.js`, on top of `__start`.
#[napi]
pub struct CommerceEventSubscription {
    inner: std::sync::Mutex<Option<stateset_embedded::EventSubscription>>,
    allowed_event_types: Option<HashSet<String>>,
    cancel: Arc<tokio::sync::Notify>,
    closed: Arc<AtomicBool>,
    callback: std::sync::Mutex<Option<EventCallback>>,
}

impl CommerceEventSubscription {
    fn new(
        inner: stateset_embedded::EventSubscription,
        allowed_event_types: Option<HashSet<String>>,
    ) -> Self {
        Self {
            inner: std::sync::Mutex::new(Some(inner)),
            allowed_event_types,
            cancel: Arc::new(tokio::sync::Notify::new()),
            closed: Arc::new(AtomicBool::new(false)),
            callback: std::sync::Mutex::new(None),
        }
    }
}

#[napi]
impl CommerceEventSubscription {
    /// Start delivery to `callback`. Each event arrives as a JSON object with
    /// an `event_type` field; `null` means the stream has ended. Internal —
    /// `index.js` calls this once, lazily, and exposes `recv()` and async
    /// iteration on top.
    #[napi(js_name = "__start", ts_args_type = "callback: (event: any | null) => void")]
    pub fn start(&self, env: Env, callback: JsFunction) -> Result<()> {
        let subscription = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .ok_or_else(|| {
                coded(ErrCode::PreconditionFailed, "Subscription delivery already started")
            })?;
        let mut tsfn: EventCallback =
            callback.create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))?;
        tsfn.unref(&env)?;
        *self.callback.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(tsfn.clone());

        let allowed = self.allowed_event_types.clone();
        let cancel = self.cancel.clone();
        let closed = self.closed.clone();
        napi::bindgen_prelude::spawn(async move {
            let mut subscription = subscription;
            loop {
                let next = tokio::select! {
                    biased;
                    () = cancel.notified() => None,
                    event = subscription.recv() => event,
                };
                let Some(event) = next else {
                    closed.store(true, Ordering::SeqCst);
                    tsfn.call(None, ThreadsafeFunctionCallMode::NonBlocking);
                    break;
                };
                let event_type = event.event_type().to_string();
                if let Some(allowed) = &allowed {
                    if !allowed.contains(&event_type) {
                        continue;
                    }
                }
                let Ok(mut value) = serde_json::to_value(&event) else {
                    continue;
                };
                // A stable `event_type` field for CLI/tools (the serde tag is `type`).
                if let serde_json::Value::Object(ref mut map) = value {
                    map.insert("event_type".to_string(), serde_json::Value::String(event_type));
                }
                tsfn.call(Some(value), ThreadsafeFunctionCallMode::NonBlocking);
            }
        });
        Ok(())
    }

    /// End the stream. A pending `recv()` resolves `null`, and so does every
    /// later one. Idempotent.
    #[napi]
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.cancel.notify_one();
    }

    /// Whether the stream has ended, by `close()` or because the engine closed.
    #[napi(getter)]
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Keep the process alive while this subscription is open. Off by
    /// default: an open subscription alone never prevents exit.
    #[napi(js_name = "ref")]
    pub fn ref_(&self, env: Env) -> Result<()> {
        if let Some(tsfn) =
            self.callback.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_mut()
        {
            tsfn.refer(&env)?;
        }
        Ok(())
    }

    /// Undo `ref()`.
    #[napi]
    pub fn unref(&self, env: Env) -> Result<()> {
        if let Some(tsfn) =
            self.callback.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_mut()
        {
            tsfn.unref(&env)?;
        }
        Ok(())
    }
}
