## Connect the cluster

Go to Digital Ocean Dashboard > Kubernetes Clusters > {Your Cluster} > More > Download Config ([like this](https://web.tresorit.com/l#TC88wCaQo01aDGM9SttIDA))

Save it to `etc/kube/kubeconfig.yaml`

For convenience of testing, add `export KUBECONFIG=/path/to/etc/kube/kubeconfig.yaml` to `~/.bash_profile`

Now you can check your setup:

```
kubectl config view
kubectl get deployments
kubectl get pods
kubectl get rs
```

## Secrets

### Create

```
kubectl create secret generic franklin-secret --from-file=./etc/env/prod/.env
kubectl create secret generic franklin-secret --from-file=./env.txt --dry-run -o yaml | kubectl apply -f -
kubectl create secret generic franklin-secret --dry-run -o yaml

```

### Misc

kubectl set env --from=configmap/myconfigmap deployment/myapp
