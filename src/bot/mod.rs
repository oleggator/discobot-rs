use std::sync::Arc;

use poise::CreateReply;
use reqwest::Client as HttpClient;
use serenity::all as serenity;
use serenity::all::GatewayIntents;
use songbird::events::{Event, EventContext, EventHandler as VoiceEventHandler, TrackEvent};
use songbird::input::YoutubeDl;
use tracing::{error, info};

pub struct Bot {
    client: serenity::Client,
}

impl Bot {
    pub async fn new(token: &str) -> Self {
        // Create our songbird voice manager
        let manager = songbird::Songbird::serenity();

        // Configure our command framework
        let options = poise::FrameworkOptions {
            commands: vec![join(), leave(), queue(), skip(), stop()],
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

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, UserData, Error>;
type CommandResult = Result<(), Error>;

struct Handler;

#[serenity::async_trait]
impl serenity::EventHandler for Handler {
    async fn ready(&self, _: serenity::Context, ready: serenity::Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

#[poise::command(slash_command, guild_only)]
async fn join(ctx: Context<'_>) -> CommandResult {
    let (guild_id, channel_id) = {
        let guild = ctx.guild().unwrap();
        let channel_id = guild
            .voice_states
            .get(&ctx.author().id)
            .and_then(|voice_state| voice_state.channel_id);

        (guild.id, channel_id)
    };

    let Some(connect_to) = channel_id else {
        if let Err(why) = ctx.reply("Not in a voice channel").await {
            error!("Error sending message: {:?}", why);
        }
        return Ok(());
    };

    let manager = ctx.data().songbird.as_ref();
    if let Ok(handler_lock) = manager.join(guild_id, connect_to).await {
        // Attach an event handler to see notifications of all track errors.
        let mut handler = handler_lock.lock().await;
        handler.add_global_event(TrackEvent::Error.into(), TrackErrorNotifier);
    }

    if let Err(why) = ctx.say("Hiiiiiii!!!!").await {
        error!("Error sending message: {:?}", why);
    }

    Ok(())
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

#[poise::command(slash_command, guild_only)]
async fn leave(ctx: Context<'_>) -> CommandResult {
    let guild_id = ctx.guild_id().unwrap();
    let manager = ctx.data().songbird.as_ref();

    if manager.get(guild_id).is_none() {
        if let Err(why) = ctx.reply("Not in a voice channel").await {
            error!("Error sending message: {:?}", why);
        }

        return Ok(());
    }

    if let Err(e) = manager.remove(guild_id).await {
        if let Err(why) = ctx.say(format!("Failed: {e:?}")).await {
            error!("Error sending message: {:?}", why);
        }
    }

    if let Err(why) = ctx.say("Left voice channel").await {
        error!("Error sending message: {:?}", why);
    }

    Ok(())
}

#[poise::command(slash_command, guild_only)]
async fn queue(ctx: Context<'_>, url: String) -> CommandResult {
    let do_search = !url.starts_with("http");

    let guild_id = ctx.guild_id().unwrap();
    let data = ctx.data();

    let message = ctx.reply("Fetching...").await.unwrap();

    let msg = if let Some(handler_lock) = ctx.data().songbird.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        let src = if do_search {
            YoutubeDl::new_search(data.http.clone(), url)
        } else {
            YoutubeDl::new(data.http.clone(), url)
        };
        handler.enqueue_input(src.into()).await;

        CreateReply::default().content("Queued song").reply(true)
    } else {
        CreateReply::default()
            .content("Not in a voice channel to play in")
            .reply(true)
    };

    if let Err(why) = message.edit(ctx, msg).await {
        error!("Error editing message: {:?}", why);
    }

    Ok(())
}

#[poise::command(slash_command, guild_only)]
async fn skip(ctx: Context<'_>) -> CommandResult {
    let guild_id = ctx.guild_id().unwrap();

    let message = ctx.reply("Skipping...").await.unwrap();

    let msg = if let Some(handler_lock) = ctx.data().songbird.get(guild_id) {
        let handler = handler_lock.lock().await;
        let _ = handler.queue().skip();

        CreateReply::default().content("Skipped").reply(true)
    } else {
        CreateReply::default()
            .content("Not in a voice channel to play in")
            .reply(true)
    };

    if let Err(why) = message.edit(ctx, msg).await {
        error!("Error editing message: {:?}", why);
    }

    Ok(())
}

#[poise::command(slash_command, guild_only)]
async fn stop(ctx: Context<'_>) -> CommandResult {
    let guild_id = ctx.guild_id().unwrap();

    let message = ctx.reply("Stopping...").await.unwrap();

    let msg = if let Some(handler_lock) = ctx.data().songbird.get(guild_id) {
        let handler = handler_lock.lock().await;
        handler.queue().stop();

        CreateReply::default().content("Stopped").reply(true)
    } else {
        CreateReply::default()
            .content("Not in a voice channel to play in")
            .reply(true)
    };

    if let Err(why) = message.edit(ctx, msg).await {
        error!("Error editing message: {:?}", why);
    }

    Ok(())
}
