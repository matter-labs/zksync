//! Supervisor is part of leader election procedure within server replicas.
//! It participates in regular pod lifetime and replicas rolling updates.
//! Use #1, pod lifecycle:
//! Whenever pod is deleted, it is removed from leader election by supervisor.
//! Whenever leader pod is having terminated status, it bails and next candidate becomes leader.
//! Use #2,
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
    // We are interested in events related to the leader pod, but watch events of all pods
    // in the same namespace. This is due to limitations of the library. And to keep things simpler too.
    let resource = Resource::namespaced::<Pod>(&namespace);
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
            if let Some(container_statuses) = &status.container_statuses {
                if let Some(last_state) = &container_statuses[0].last_state {
                    if last_state.terminated != None {
                        bail_if_leader(&storage, name);
                    }
                }
            }
        }
        WatchEvent::Deleted(o) => {
            let name = Meta::name(&o);
            debug!("Deleted Pod: {}", name);
            info!("Bailing pod because it is being deleted: {}", name);
            storage
                .leader_election_schema()
                .bail(&name, None)
                .expect("failed to bail deleted pod from election");
        }
        WatchEvent::Error(e) => {
            warn!("Error event: {:?}", e);
        }
    }
    Ok(())
}

fn bail_if_leader(storage: &StorageProcessor, name: String) {
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
