// Built-in uses
use std::collections::HashSet;
// External uses
use actix::prelude::*;
// Workspace uses
// Local uses
use crate::messages::*;
use crate::subscriber::Subscriber;

/// The actor responsible for maintaining the set of connections.
#[derive(Debug, Default)]
pub struct ServerMonitor {
    addrs: HashSet<Addr<Subscriber>>,
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
