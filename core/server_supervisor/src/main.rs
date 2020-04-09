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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env::set_var("RUST_LOG", "info,kube=debug");
    env_logger::init();
    let client = Client::from(Configuration::infer().await?);
    let namespace = env::var("NAMESPACE").unwrap_or_else(|_| "default".into());
    let resource = Resource::namespaced::<Pod>(&namespace);
    let inf = Informer::new(client, ListParams::default(), resource);

    loop {
        let mut pods = inf.poll().await?.boxed();

        while let Some(event) = pods.try_next().await? {
            handle_pod(event)?;
        }
    }
}

// This function lets the app handle an event from kube
fn handle_pod(ev: WatchEvent<Pod>) -> anyhow::Result<()> {
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
            info!("Added Pod: {} (containers={:?})", name, containers);
        }
        WatchEvent::Modified(o) => {
            let name = Meta::name(&o);
            let owner = &Meta::meta(&o).owner_references.clone().unwrap()[0];
            let phase = o.status.unwrap().phase.unwrap();
            info!(
                "Modified Pod: {} (phase={}, owner={})",
                name, phase, owner.name
            );
        }
        WatchEvent::Deleted(o) => {
            info!("Deleted Pod: {}", Meta::name(&o));
        }
        WatchEvent::Error(e) => {
            warn!("Error event: {:?}", e);
        }
    }
    Ok(())
}
