## Connect the cluster

Go to Digital Ocean Dashboard > Kubernetes Clusters > {Your Cluster} > More > Download Config 
https://cloud.digitalocean.com/kubernetes/clusters?i=ba0188

([like this](https://web.tresorit.com/l#TC88wCaQo01aDGM9SttIDA))

Save it to `etc/kube/kubeconfig.yaml`

For convenience of testing, add `export KUBECONFIG=/path/to/etc/kube/kubeconfig.yaml` to `~/.bash_profile`

Now you can check your setup:

```
kubectl config view
kubectl get deployments
kubectl get nodes --show-labels
kubectl get pods -o wide
```

## Secrets

View secret:

```kubectl get secret franklin-secret -o yaml```

Misc:

```kubectl set env --from=configmap/myconfigmap deployment/myapp```

## Scale provers

kubectl scale deployments/prover --replicas=3
