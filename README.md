# Introduction

This project was inspired by this article: https://truelayer.com/blog/grpc-load-balancing-in-rust

We needed to test gRPC's stability in a containerized environment, whereby a number of servers and clients are spawned and we can test whether the clients can recover from servers being switched off. 

We use tonic library for our gRPC integration in rust. ginepro is used for DNS sidecar loadbalancing. This is a cool integration for K8s, but here we show examples in docker-compose. 

# How To Run 

1. Make sure you have docker and docker compose available. Docker desktop is great for this! 
2. Build the container: ```docker build -t grpc-test -f ./DockerFile .```
3. Run it in docker-compose: ```docker build -t grpc-test -f ./DockerFile .```
