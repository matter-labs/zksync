// Built-in uses
use std::collections::HashSet;
// External uses
use actix::prelude::*;
use actix_web::dev::Server;
// Workspace uses
// Local uses
use crate::messages::*;
use crate::subscriber::Subscriber;

/// The actor responsible for maintaining the set of connections.
#[derive(Debug, Default)]
pub struct ServerMonitor {
    addrs: HashSet<Addr<Subscriber>>,
    server_handle: Option<Server>,
}

impl ServerMonitor {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Actor for ServerMonitor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.set_mailbox_capacity(1 << 32);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        vlog::warn!("ServerMonitor actor has stopped");
    }
}

impl Handler<RegisterSubscriber> for ServerMonitor {
    type Result = ();

    fn handle(&mut self, request: RegisterSubscriber, _ctx: &mut Self::Context) {
        self.addrs.insert(request.0);
    }
}

impl Handler<RemoveSubscriber> for ServerMonitor {
    type Result = ();

    fn handle(&mut self, request: RemoveSubscriber, _ctx: &mut Self::Context) {
        self.addrs.remove(&request.0);
    }
}

impl Handler<NewEvents> for ServerMonitor {
    type Result = ();

    fn handle(&mut self, msg: NewEvents, ctx: &mut Self::Context) {
        if msg.0.as_ref().is_empty() {
            vlog::info!("Server monitor received empty array of events");
            return;
        }
        for addr in self.addrs.iter().cloned() {
            addr.send(msg.clone())
                .into_actor(self)
                .map(move |response, act, _| match response {
                    Ok(()) => {}
                    Err(MailboxError::Timeout) => {}
                    Err(MailboxError::Closed) => {
                        // The corresponding `Subscriber` actor finished its work,
                        // but didn't notify the monitor about it.
                        // Remove his address.
                        act.addrs.remove(&addr);
                    }
                })
                .spawn(ctx);
        }
    }
}

impl Handler<RegisterServerHandle> for ServerMonitor {
    type Result = ();

    fn handle(&mut self, msg: RegisterServerHandle, _ctx: &mut Self::Context) {
        self.server_handle.replace(msg.0);
    }
}

impl Handler<Shutdown> for ServerMonitor {
    type Result = ();

    fn handle(&mut self, _msg: Shutdown, ctx: &mut Self::Context) {
        // Since actix can't gracefully shutdown the WebSocket
        // server on its own, we have to send this message to
        // all subscribers, wait for them to close their connections
        // and only then stop the server and the context.
        let server_handle = self.server_handle.take().unwrap();
        let addrs = self.addrs.clone();
        async move {
            // Stop accepting new connections.
            server_handle.pause().await;
            for addr in addrs {
                let _ = addr.send(Shutdown).await;
            }
            server_handle.stop(false).await;
        }
        .into_actor(self)
        .map(|_, _, ctx| ctx.stop())
        .wait(ctx);
    }
}
