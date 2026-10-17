use std::time::Duration;

use poise::CreateReply;
use songbird::events::TrackEvent;
use songbird::input::{Compose, YoutubeDl};
use tracing::{error, warn};

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, super::UserData, Error>;
type CommandResult = Result<(), Error>;

/// How long yt-dlp gets to find a track before `/queue` gives up.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(30);

#[poise::command(slash_command, guild_only)]
pub async fn join(ctx: Context<'_>) -> CommandResult {
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
        handler.add_global_event(TrackEvent::Error.into(), super::TrackErrorNotifier);
    }

    if let Err(why) = ctx.say("Hiiiiiii!!!!").await {
        error!("Error sending message: {:?}", why);
    }

    Ok(())
}

#[poise::command(slash_command, guild_only)]
pub async fn leave(ctx: Context<'_>) -> CommandResult {
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
pub async fn queue(ctx: Context<'_>, url: String) -> CommandResult {
    let do_search = !url.starts_with("http");

    let guild_id = ctx.guild_id().unwrap();
    let data = ctx.data();

    let message = ctx.reply("Fetching...").await.unwrap();

    let msg = if let Some(handler_lock) = ctx.data().songbird.get(guild_id) {
        let mut src = if do_search {
            YoutubeDl::new_search(data.http.clone(), url)
        } else {
            YoutubeDl::new(data.http.clone(), url)
        };

        // Look the track up before taking the call lock. Enqueueing needs the track's metadata
        // and would otherwise run yt-dlp while holding the lock, blocking /skip, /stop and
        // /leave; the result is cached on `src`, so songbird reuses it.
        match tokio::time::timeout(LOOKUP_TIMEOUT, src.aux_metadata()).await {
            Ok(Ok(_)) => {
                let mut handler = handler_lock.lock().await;
                handler.enqueue_input(src.into()).await;

                CreateReply::default().content("Queued song").reply(true)
            }
            Ok(Err(why)) => {
                warn!("Error looking up track: {:?}", why);

                CreateReply::default()
                    .content("Couldn't load that track")
                    .reply(true)
            }
            Err(_) => CreateReply::default()
                .content("Timed out loading that track")
                .reply(true),
        }
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
pub async fn skip(ctx: Context<'_>) -> CommandResult {
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
pub async fn stop(ctx: Context<'_>) -> CommandResult {
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
