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

#[derive(Debug)]
pub struct Subscriber {
    filters: Option<SubscriberFilters>,
    monitor: Addr<ServerMonitor>,
}

impl Subscriber {
    pub fn new(monitor: Addr<ServerMonitor>) -> Self {
        Self {
            filters: None,
            monitor,
        }
    }

    fn shutdown(&mut self, ctx: &mut <Self as Actor>::Context) {
        let request = RemoveSubscriber(ctx.address());
        self.monitor
            .send(request)
            .into_actor(self)
            .map(|response, _, ctx| {
                if let Err(err) = response {
                    vlog::error!("Couldn't remove the subscriber, reason: {:?}", err);
                }
                ctx.stop();
            })
            .wait(ctx);
    }
}

impl Actor for Subscriber {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.set_mailbox_capacity(1 << 10);

        let request = RegisterSubscriber(ctx.address());
        self.monitor
            .send(request)
            .into_actor(self)
            .map(|response, _, ctx| {
                if let Err(err) = response {
                    vlog::error!("Couldn't register new subscriber, reason: {:?}", err);
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
                if self.filters.is_some() {
                    return;
                }
                match serde_json::from_str(&text) {
                    Ok(filters) => {
                        self.filters = Some(filters);
                    }
                    Err(err) => {
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
