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

# The schema is seeded by migration 01, edited in place, and sqlx stores its
# checksum, so an existing database cannot be migrated forward: drop the volume.
# pg_isready goes over TCP because the image's init-phase server listens on the
# socket only, so a socket check passes before the real server is up.
# A running gateway must be restarted afterwards: it hydrates only at startup.
.PHONY: db-reset
db-reset: ## Recreate the local Postgres from scratch and migrate (destroys its data)
	docker compose down -v
	docker compose up -d
	@for i in $$(seq 60); do \
	  docker compose exec -T postgres pg_isready -h 127.0.0.1 -q && exit 0; \
	  sleep 1; \
	done; \
	echo "db-reset: Postgres not ready after 60s" >&2; exit 1
	sqlx migrate run

.PHONY: prepare
prepare: ## Regenerate .sqlx offline data (needs a running database)
	cargo sqlx prepare -- --all-targets

# git status --porcelain, not git diff --exit-code: diff ignores untracked
# files, and a query whose text changed lands in .sqlx/ as a new, untracked file.
.PHONY: check-sqlx
check-sqlx: ## Regenerate .sqlx and fail if it differs from git (needs Postgres)
	cargo sqlx prepare -- --all-targets
	@if [ -n "$$(git status --porcelain -- .sqlx)" ]; then \
	  echo "check-sqlx: .sqlx/ differs from git; commit the regenerated files" >&2; \
	  git status --porcelain -- .sqlx >&2; \
	  exit 1; \
	fi

.PHONY: check
check: ## Type-check everything without a database
	SQLX_OFFLINE=true cargo check --all-targets

.PHONY: test
test: ## Run the unit tests (no database needed)
	SQLX_OFFLINE=true cargo test

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

.PHONY: deploy
deploy: ## Apply the manifests to the kind cluster
	kubectl apply -f k8s/

.PHONY: undeploy
undeploy: ## Remove the netgw namespace and everything in it
	kubectl delete namespace netgw --ignore-not-found

# The initdb ConfigMap runs only against an empty data directory, so a schema
# change needs a fresh PVC. The gateway hydrates only at startup, so restart it
# once the new database is in place.
.PHONY: cluster-db-reset
cluster-db-reset: ## Recreate the cluster Postgres and its PVC, then restart the gateway (destroys its data)
	kubectl delete statefulset postgres -n netgw --ignore-not-found
	kubectl delete pvc data-postgres-0 -n netgw --ignore-not-found
	kubectl apply -f k8s/
	kubectl rollout status statefulset/postgres -n netgw
	kubectl rollout restart deployment/gateway -n netgw
	kubectl rollout status deployment/gateway -n netgw

.PHONY: status
status: ## Show what is running in the netgw namespace
	kubectl get all,pvc -n netgw

.PHONY: logs
logs: ## Tail the gateway pod logs
	kubectl logs -n netgw -l app=gateway -f --tail=50

.PHONY: cluster-info
cluster-info: ## Show cluster and node status
	kubectl cluster-info --context kind-$(CLUSTER)
	kubectl get nodes -o wide
