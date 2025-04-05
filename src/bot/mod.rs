mod commands;

use std::sync::Arc;

use reqwest::Client as HttpClient;
use serenity::all as serenity;
use serenity::all::GatewayIntents;
use songbird::events::{Event, EventContext, EventHandler as VoiceEventHandler};
use tracing::info;

pub struct Bot {
    client: serenity::Client,
}

impl Bot {
    pub async fn new(token: &str) -> Self {
        // Create our songbird voice manager
        let manager = songbird::Songbird::serenity();

        // Configure our command framework
        let options = poise::FrameworkOptions {
            commands: vec![
                commands::join(),
                commands::leave(),
                commands::queue(),
                commands::skip(),
                commands::stop(),
            ],
            ..Default::default()
        };

        // We have to clone our voice manager's Arc to share it between serenity and our user data.
        let manager_clone = Arc::clone(&manager);
        let framework = poise::Framework::new(options, |_, _, _| {
            Box::pin(async {
                Ok(UserData {
                    http: HttpClient::new(),
                    songbird: manager_clone,
                })
            })
        });

        let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
        let client = serenity::Client::builder(token, intents)
            .voice_manager_arc(manager)
            .event_handler(Handler)
            .framework(framework)
            .await
            .expect("Err creating client");

        Bot { client }
    }

    pub async fn start(&mut self) -> Result<(), serenity::Error> {
        self.client.start().await
    }
}

struct UserData {
    http: HttpClient,
    songbird: Arc<songbird::Songbird>,
}

struct Handler;

#[serenity::async_trait]
impl serenity::EventHandler for Handler {
    async fn ready(&self, _: serenity::Context, ready: serenity::Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

struct TrackErrorNotifier;

#[serenity::async_trait]
impl VoiceEventHandler for TrackErrorNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(track_list) = ctx {
            for (state, handle) in *track_list {
                info!(
                    "Track {:?} encountered an error: {:?}",
                    handle.uuid(),
                    state.playing
                );
            }
        }

        None
    }
}
