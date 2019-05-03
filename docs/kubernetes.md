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

0. Make sure to have file `etc/kube/franklin.yaml`

1. Update keys in .sol files, then deploy contracts:

```
deploy-contracts prod
```

2. Upload the .pk key files to DO Spaces:

https://cloud.digitalocean.com/spaces/keys?i=ba0188

3. Build and push your images to DockerHub:

```
make push
```

4. Deploy kubernetes and/or update env vars

```
deploy-kube prod
```

5. Scale nodes:

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

https://kubernetes.io/docs/reference/kubectl/cheatsheet/

```
kubectl get pods -o wide
kubectl logs -f <pod id>
```

### Secrets

View secret:

```kubectl get secret franklin-secret -o yaml```

Misc:

```kubectl set env --from=configmap/myconfigmap deployment/myapp```
