## Connect the cluster

Go to Digital Ocean Dashboard > Kubernetes Clusters > {Your Cluster} > More > Download Config 
https://cloud.digitalocean.com/kubernetes/clusters?i=ba0188

![screenshot](kube-config.png)

Save it to `etc/kube/kubeconfig.yaml`

For convenience of testing, add `export KUBECONFIG=/path/to/etc/kube/kubeconfig.yaml` to `~/.bash_profile`

Now you can check your setup:

```
kubectl config view
```

## Deploy

1. Upload the keys to DO Spaces:

https://cloud.digitalocean.com/spaces/keys?i=ba0188

2. Build and push your images to DockerHub:

```
make push
```

3. Scale nodes:

```
kubectl scale deployments/server --replicas=1
kubectl scale deployments/prover --replicas=3
```

## Check status:

1. Nodes:
```
kubectl get pods
```

2. Web server:
https://api1.mattr.network/api/v0.1/status

## Misc

### Commands

```
kubectl get deployments
kubectl get nodes --show-labels
kubectl get pods -o wide
```

### Secrets

View secret:

```kubectl get secret franklin-secret -o yaml```

Misc:

```kubectl set env --from=configmap/myconfigmap deployment/myapp```
