#[macro_use]
extern crate log;
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{ListParams, Meta, Resource, WatchEvent},
    runtime::Informer,
    Client, Configuration,
};
use std::env;
use storage::StorageProcessor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let storage = StorageProcessor::establish_connection().expect("failed connect to db");
    let namespace = env::var("NAMESPACE").unwrap_or_else(|_| "default".into());
    let client = Client::from(Configuration::infer().await?);
    let resource = Resource::namespaced::<Pod>(&namespace);
    // We are interested of the events related to the leader pod, but watch event of all the pods
    // in the same namespace. This is due to limitations of the library. And for simplicity too.
    let inf = Informer::new(client, ListParams::default(), resource);

    loop {
        let mut pods = inf.poll().await?.boxed();

        while let Some(event) = pods.try_next().await? {
            handle_pod(event, &storage)?;
        }
    }
}

// This function lets the app handle an event from kube
// When receives event of the leader pod with last state being terminated, bails it from election,
// thus enforces new leader.
fn handle_pod(ev: WatchEvent<Pod>, storage: &StorageProcessor) -> anyhow::Result<()> {
    match ev {
        WatchEvent::Added(o) => {
            let name = Meta::name(&o);
            let containers = o
                .spec
                .unwrap()
                .containers
                .into_iter()
                .map(|c| c.name)
                .collect::<Vec<_>>();
            debug!("Added Pod: {} (containers={:?})", name, containers);
        }
        WatchEvent::Modified(o) => {
            let name = Meta::name(&o);
            let owner = &Meta::meta(&o).owner_references.clone().unwrap()[0];
            let status = o.status.unwrap();
            let phase = status.phase.unwrap();
            debug!(
                "Modified Pod: {} (phase={}, owner={})",
                name, phase, owner.name
            );
            if let Some(last_state) = &status.container_statuses.unwrap()[0].last_state {
                if last_state.terminated != None {
                    let current_leader = storage
                        .leader_election_schema()
                        .current_leader()
                        .expect("failed to get current leader");
                    if let Some(leader) = current_leader {
                        if leader.name == name {
                            info!(
                                "Bailing leader because pod is last known to be terminated: {}",
                                leader.name
                            );
                            storage
                                .leader_election_schema()
                                .bail(&leader.name, Some(leader.created_at))
                                .expect("failed to bail terminated leader from election");
                        }
                    }
                }
            }
        }
        WatchEvent::Deleted(o) => {
            debug!("Deleted Pod: {}", Meta::name(&o));
        }
        WatchEvent::Error(e) => {
            warn!("Error event: {:?}", e);
        }
    }
    Ok(())
}
