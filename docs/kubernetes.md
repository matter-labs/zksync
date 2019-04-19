## Connect the cluster

Go to Digital Ocean Dashboard > Kubernetes Clusters > {Your Cluster} > More > Download Config ([like this](https://web.tresorit.com/l#TC88wCaQo01aDGM9SttIDA))

Save it to `etc/kube/kubeconfig.yaml`

For convenience of testing, add `export KUBECONFIG=/path/to/etc/kube/kubeconfig.yaml` to `~/.bash_profile`

Now you can check your setup:

```
kubectl config view
kubectl get deployments
kubectl get nodes
kubectl get pods
```

## Secrets

View secret:

```kubectl get secret franklin-secret -o yaml```

Misc:

```kubectl set env --from=configmap/myconfigmap deployment/myapp```
