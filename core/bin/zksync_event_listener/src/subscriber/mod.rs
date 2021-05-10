// Built-in uses
// External uses
use actix::prelude::*;
use actix_web_actors::ws;
// Workspace uses
// Local uses
use crate::messages::{NewEvents, RegisterSubscriber, RemoveSubscriber};
use crate::monitor::ServerMonitor;
use filters::SubscriberFilters;

mod filters;

/// The WebSocket actor. Created for each connected client.
#[derive(Debug)]
pub struct Subscriber {
    /// Subscriber's events interests. Remain `None` until the client
    /// sends JSON-serialized map of filters. Before that, all incoming
    /// events will be ignored.
    filters: Option<SubscriberFilters>,
    /// The address of the [`ServerMonitor`] for registering.
    monitor: Addr<ServerMonitor>,
}

impl Subscriber {
    pub fn new(monitor: Addr<ServerMonitor>) -> Self {
        Self {
            filters: None,
            monitor,
        }
    }

    /// Remove the subscriber's address from the monitor's set and stop
    /// the execution context completely. Should be called instead of
    /// `ctx.stop()`.
    ///
    /// Note, that the close frame is expected to be sent to the client and
    /// this method rather serves the purpose of the clean-up.
    fn shutdown(&mut self, ctx: &mut <Self as Actor>::Context) {
        // Send the message and wait for the empty response on the actor's context.
        let request = RemoveSubscriber(ctx.address());
        self.monitor
            .send(request)
            .into_actor(self)
            .map(|response, _, ctx| {
                if let Err(err) = response {
                    vlog::error!("Couldn't remove the subscriber, reason: {:?}", err);
                }
                // Finally, stop the actor's context.
                ctx.stop();
            })
            .wait(ctx);
    }
}

impl Actor for Subscriber {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.set_mailbox_capacity(1 << 10);
        // Send the register message and wait for the empty response on the actor's context.
        // If we couldn't register this subscriber for some reason, close the connection
        // immediately.
        let request = RegisterSubscriber(ctx.address());
        self.monitor
            .send(request)
            .into_actor(self)
            .map(|response, _, ctx| {
                if let Err(err) = response {
                    vlog::error!("Couldn't register new subscriber, reason: {:?}", err);
                    let reason = Some(ws::CloseReason {
                        code: ws::CloseCode::Error,
                        description: None,
                    });
                    ctx.close(reason);
                    ctx.stop();
                }
            })
            .wait(ctx);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for Subscriber {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                // If the client already registered his interests,
                // ignore the message, otherwise, try to parse the text.
                if self.filters.is_some() {
                    return;
                }
                match serde_json::from_str(&text) {
                    Ok(filters) => {
                        self.filters = Some(filters);
                    }
                    Err(err) => {
                        // The client provided invalid JSON, give
                        // him the error message and close the connection.
                        let reason = Some(ws::CloseReason {
                            code: ws::CloseCode::Policy,
                            description: Some(err.to_string()),
                        });
                        ctx.close(reason);
                        self.shutdown(ctx);
                    }
                }
            }
            Ok(ws::Message::Close(reason)) => {
                // Send back the close frame.
                ctx.close(reason);
                self.shutdown(ctx);
            }
            Err(err) => {
                let reason = Some(ws::CloseReason {
                    code: ws::CloseCode::Error,
                    description: Some(err.to_string()),
                });
                ctx.close(reason);
                self.shutdown(ctx);
            }
            _ => {}
        }
    }

    fn finished(&mut self, ctx: &mut Self::Context) {
        // The client disconnected without sending the close frame.
        self.shutdown(ctx);
    }
}

impl Handler<NewEvents> for Subscriber {
    type Result = ();

    fn handle(&mut self, msg: NewEvents, ctx: &mut Self::Context) {
        let filters = match &self.filters {
            Some(filters) => filters,
            None => return,
        };
        for event in msg.0.as_ref() {
            if !filters.matches(event) {
                continue;
            }
            let json = serde_json::to_string(&event).unwrap();
            ctx.text(json);
        }
    }
}
