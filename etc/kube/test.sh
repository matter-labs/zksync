kubectl --kubeconfig="k8s-ams3-kubeconfig.yaml" get nodes
kubectl create --kubeconfig="k8s-ams3-kubeconfig.yaml" -f ./franklin.yaml
kubectl --kubeconfig="k8s-ams3-kubeconfig.yaml" get pods
kubectl describe pod
