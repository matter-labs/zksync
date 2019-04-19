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
