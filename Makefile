IMAGE   ?= grpc-network-gateway:dev
CLUSTER ?= netgw

.DEFAULT_GOAL := help

.PHONY: help
help: ## List available targets
	@grep -hE '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) \
	  | awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

# --- local development -------------------------------------------------------

.PHONY: db-up
db-up: ## Start the local Postgres used for development
	docker compose up -d

.PHONY: db-down
db-down: ## Stop the local Postgres
	docker compose stop postgres

.PHONY: migrate
migrate: ## Apply database migrations
	sqlx migrate run

.PHONY: prepare
prepare: ## Regenerate .sqlx offline data (needs a running database)
	cargo sqlx prepare -- --all-targets

.PHONY: check
check: ## Type-check everything without a database
	SQLX_OFFLINE=true cargo check --all-targets

.PHONY: run
run: ## Run the gateway against the local Postgres
	SQLX_OFFLINE=true cargo run --bin grpc_network_gateway

.PHONY: smoke
smoke: ## Exercise create/observe/delete against a running gateway
	SQLX_OFFLINE=true cargo run --quiet --example smoke_client

.PHONY: probe
probe: ## Query the gRPC health service, exit non-zero when not serving
	SQLX_OFFLINE=true cargo run --quiet --example health_probe

# --- container ---------------------------------------------------------------

.PHONY: image
image: ## Build the container image
	docker build -t $(IMAGE) .

# --- kubernetes --------------------------------------------------------------

.PHONY: cluster-up
cluster-up: ## Create the local kind cluster
	kind create cluster --config kind/cluster.yaml

.PHONY: cluster-down
cluster-down: ## Delete the local kind cluster
	kind delete cluster --name $(CLUSTER)

.PHONY: load
load: image ## Build the image and load it into the kind cluster
	# kind nodes have their own image store and never read the host's. Without
	# this the pod fails ImagePullBackOff trying to fetch a tag that only
	# exists locally.
	kind load docker-image $(IMAGE) --name $(CLUSTER)

.PHONY: cluster-info
cluster-info: ## Show cluster and node status
	kubectl cluster-info --context kind-$(CLUSTER)
	kubectl get nodes -o wide
