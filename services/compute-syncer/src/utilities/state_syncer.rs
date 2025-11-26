use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::apps::v1::Deployment;
use kube::runtime::{reflector, watcher};
use kube::{Api, Client, ResourceExt};
use redis::AsyncCommands;
use redis::aio::MultiplexedConnection;
use shared::utilities::errors::AppError;
use tracing::info;

pub async fn state_syncer(
    client: Client,
    mut connection: MultiplexedConnection,
) -> Result<(), AppError> {
    let api = Api::<Deployment>::all(client);
    let (_reader, writer) = reflector::store();

    let mut stream = reflector(writer, watcher(api, Default::default())).boxed();

    while let Some(event) = stream.try_next().await? {
        use kube::runtime::watcher::Event;

        match event {
            Event::Apply(d) | Event::InitApply(d) => {
                info!(
                    "Syncing deployment: {}/{}",
                    d.namespace().unwrap_or("default".to_string()),
                    d.name_any()
                );
                update_redis(&mut connection, &d).await?;
            }
            Event::Delete(d) => {
                info!(
                    "Deleting deployment from cache: {}/{}",
                    d.namespace().unwrap_or("default".to_string()),
                    d.name_any()
                );
                let key = format!(
                    "deploy:{}:{}",
                    d.namespace().unwrap_or("default".to_string()),
                    d.name_any()
                );
                let _: u64 = connection.del(&key).await?;
            }
            Event::Init => info!("Starting full sync..."),
            Event::InitDone => info!("Full sync complete"),
        }
    }

    Ok(())
}

async fn update_redis(
    connection: &mut MultiplexedConnection,
    deployment: &Deployment,
) -> Result<(), AppError> {
    let namespace = deployment.namespace().unwrap_or("default".to_owned());
    let name = deployment.name_any();
    let key = format!("deploy:{}:{}", namespace, name);

    let state = serde_json::json!({
        "name": name,
        "namespace": namespace,
        "replicas": deployment.spec.as_ref().and_then(|s| s.replicas).unwrap_or(1),
        "ready_replicas": deployment.status.as_ref().and_then(|s| s.ready_replicas).unwrap_or(0),
    });

    let _: u64 = connection.set_ex(&key, state.to_string(), 600).await?; // 10 min TTL

    Ok(())
}
