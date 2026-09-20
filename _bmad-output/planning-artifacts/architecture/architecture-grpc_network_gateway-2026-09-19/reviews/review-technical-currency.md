# Technical Currency Review — ARCHITECTURE-SPINE.md

**Artifact:** `_bmad-output/planning-artifacts/architecture/architecture-grpc_network_gateway-2026-09-19/ARCHITECTURE-SPINE.md`
**Lens:** technical accuracy / currency — was every committed decision web-researched or reality-checked, or asserted from training data?
**Reviewed:** 2026-09-20
**Method:** every version claim checked against a live upstream source (crates.io API, GitHub Releases API, upstream go.mod, helm.sh docs and HIPs, KEP-5573); every environment claim checked against this host directly (`mount`, `/proc/cmdline`, `command -v`); repo claims checked against `Cargo.toml`, `Cargo.lock`, `Dockerfile`, `kind/cluster.yaml`, `Makefile`, `build.rs`.

**Verdict: the Stack table is unusually well-sourced — 15 of 16 rows verified exactly right — but the one environment decision that gates all of Phase C (AD-11's cgroup premise) is asserted, not reality-checked, and is false on this machine.**

---

## 1. Findings

### F-1 — CRITICAL — AD-11's cgroup premise is false on this host; the retired pin is still load-bearing

AD-11 states:

> node image pinned **by digest** at v1.37.0 — the host runs the unified cgroup hierarchy
> (`kernelCommandLine = cgroup_no_v1=all` in `.wslconfig`), so the 1.33 pin is retired.

This is the one claim in the document that could have been settled by a single command, and it was not. Checked live on this host:

```
$ mount | grep cgroup
tmpfs on /sys/fs/cgroup type tmpfs (ro,...)
cgroup2 on /sys/fs/cgroup/unified type cgroup2 ...
cgroup on /sys/fs/cgroup/cpuset type cgroup ...      <- v1
cgroup on /sys/fs/cgroup/memory type cgroup ...      <- v1
... (14 more v1 controllers)

$ cat /sys/fs/cgroup/cgroup.controllers
ABSENT -> not unified v2

$ cat /proc/cmdline
initrd=\initrd.img WSL_ROOT_INIT=1 panic=-1 nr_cpus=8 hv_utils.timesync_implicit=1 \
console=hvc0 debug pty.legacy_count=0 WSL_ENABLE_CRASH_DUMP=1
```

The host is on the **legacy hybrid hierarchy**, not the unified one. `cgroup_no_v1` does not appear in `/proc/cmdline`, so the `.wslconfig` edit was either never made, never followed by `wsl --shutdown`, or is not taking effect. The full v1 controller set is mounted and `/sys/fs/cgroup/cgroup.controllers` — the file that only exists under a unified v2 root — is absent.

Consequences:

- `kindest/node:v1.37.0` **will not boot here.** The kubelet's `FailCgroupV1` check fires and the node never becomes ready. AD-11 as written makes `make cluster-up` fail on the machine the PRD's SM-2 is measured on.
- The repository still contradicts the spine. `kind/cluster.yaml` pins `kindest/node:v1.33.1` **by tag, not digest**, and its comment says the opposite of AD-11:

  ```yaml
  # The node image is pinned because this host (WSL2) mounts cgroup v1, and the
  # kubelet from Kubernetes 1.36 onward refuses to start on it. v1.33 still
  # tolerates cgroup v1. Drop this pin once the host runs the unified hierarchy
  # (add `kernelCommandLine = cgroup_no_v1=all` to .wslconfig, then wsl --shutdown).
  ```

  The spine has recorded the *intended* follow-up of that comment as a completed fact.

**Fix:** either (a) actually perform the cutover and re-verify with `cat /sys/fs/cgroup/cgroup.controllers` before restating AD-11, or (b) restate AD-11 as a conditional — the v1.37.0 digest pin is the target, contingent on a cgroup v2 host, with a named fallback. Do not leave a spine invariant depending on an unperformed host change.

---

### F-2 — HIGH — `prost 0.14.6` does not exist; the row collapses four crates onto one wrong version

Stack row: `tonic · prost · tonic-prost · tonic-health | 0.14.6`

Verified on crates.io (2026-09-20):

| Crate | Spine says | crates.io `max_stable_version` | Verdict |
| --- | --- | --- | --- |
| `tonic` | 0.14.6 | **0.14.6** (2026-05-07) | correct |
| `tonic-prost` | 0.14.6 | **0.14.6** (2026-05-07) | correct |
| `tonic-health` | 0.14.6 | **0.14.6** (2026-05-07) | correct |
| `prost` | 0.14.6 | **0.14.4** (2026-06-07) | **wrong — 0.14.6 was never published** |

`prost` is a separate project from `tonic` and does not share its patch cadence. The repo agrees with reality and disagrees with the spine — `Cargo.lock` has `prost 0.14.4`.

This is the signature of a version asserted by pattern-matching the neighbouring rows rather than looked up. Split the row, or state the shared version as `tonic family 0.14.6, prost 0.14.4`.

---

### F-3 — HIGH — The cgroup cutover release is **1.35**, not "1.36+", and the stated fallback does not exist in kind v0.33.0

AD-11 and `kind/cluster.yaml` both assert that "the kubelet from Kubernetes 1.36 onward refuses to start" on cgroup v1. Checked against the authoritative source, **KEP-5573 (Remove cgroup v1 support)**:

> beta will prevent kubelet from starting on a cgroup v1 node … The removal will be done no earlier than 1.38.

`FailCgroupV1` graduated to **beta with default `true` in v1.35**. Vendor confirmation agrees (SUSE KB: *"Nodes with cgroup v1 fail to start by default in Kubernetes v1.35+"*). Full code removal is no earlier than 1.38 and still undetermined.

The off-by-one matters because it changes what the fallback ladder is. kind v0.33.0 ships exactly four prebuilt node images (from the release body):

- `v1.37.0@sha256:a1ed56cfb0e7b93589bdf97c8cd566405a265939e3620fc4f5de89adff580ae5` (default)
- `v1.36.4@sha256:099e049362a1526b2db71494e1947aae99bd16290d7c895f2b7ea312e3cbfaed`
- `v1.35.8@sha256:07b2536e30b803ed61d1677a79df6115f798ce64c80f9e22f6ed45afd09323c0`
- `v1.34.11@sha256:44e222ee2132dab25ff87301682f89eb82c7880ea3a1bf543bfe9708fd08d67d`

So on the actual (cgroup v1) host:

- v1.37.0, v1.36.4 **and v1.35.8 all fail** — not just 1.36+.
- **`v1.34.11` is the only kind v0.33.0 prebuilt image that boots.**
- The repo's current `v1.33.1` pin is **not a kind v0.33.0 prebuilt image at all** — it belongs to an older kind release and is unsupported by the installed `kind v0.33.0`.

A third escape hatch exists and is not mentioned anywhere: `failCgroupV1: false` in the kubelet config, settable through kind's `kubeadmConfigPatches`. AD-11 should name it, because it is the option that preserves the v1.37.0 pin without a host reboot.

---

### F-4 — MEDIUM — The Rust row asserts a toolchain pin the repo does not contain, and a tag the Dockerfile does not use

Stack row: `Rust | 1.97.1 (toolchain and rust:1.97-slim builder; 1.98.1 is current)`

- **`1.98.1` is current** — confirmed, released 2026-09-01 (blog post 2026-09-03), a patch fixing a vtable miscompilation in 1.98.0. Next stable 1.99.0 is due 2026-10-01. Correct.
- **`1.97.1` exists** — confirmed on releases.rs, and matches this host: `rustc 1.97.1 (8bab26f4f 2026-07-14)`. Correct.
- **"toolchain" is not pinned.** There is **no `rust-toolchain.toml` or `rust-toolchain` file in the repository.** Nothing in the repo pins 1.97.1; that number is simply whatever happens to be installed on this machine today. The spine states it as a committed decision.
- **The builder tag is not `rust:1.97-slim`.** The Dockerfile uses `rust:1.97-slim-bookworm` pinned by digest (`sha256:2775a09d…`). The digest pin is good practice and is worth crediting, but the spine names a different tag.

Also: `Cargo.toml` declares loose carets (`tonic = "0.14"`, `prost = "0.14"`, `tokio = "1.52"`), so the exact versions in the Stack table are properties of `Cargo.lock`, not of the manifest. Under "Verified against live sources" that distinction should be explicit — a fresh `cargo update` moves them.

---

### F-5 — MEDIUM — `tracing-subscriber` is in the Stack table and load-bearing in AD-9, but is not a dependency

`Cargo.toml` `[dependencies]` in full: `openssl`, `tokio`, `tonic`, `prost`, `tonic-prost`, `sqlx`, `tracing`, `dotenvy`, `tonic-health`. **`tracing-subscriber` is absent.** Without a subscriber installed, every `tracing` macro in the gateway is a no-op and emits nothing.

- Spine Stack: `tracing + tracing-subscriber | 0.1 / 0.3`
- AD-9: *"Rust uses `tracing` with `tracing-subscriber`; no `println!` survives E1."*

The `0.1 / 0.3` version pair is correct in the abstract — live: `tracing` latest 0.1.44 (repo lock: 0.1.44), `tracing-subscriber` latest **0.3.23** (2026-03-13). But the Stack table presents a dependency the project does not have as though it were verified present. AD-9's framing ("which is the state today") shows the gap was known for `println!`; the missing subscriber crate was not caught. Either mark the row as *to be added in E1* or add the dependency.

---

### F-6 — LOW — kubebuilder v4.16.0 scaffolds controller-runtime **v0.25.0**, not v0.25.1

Both versions are individually correct and current:

- kubebuilder **v4.16.0**, released 2026-09-10 — latest. Its release notes list *"controller-tools v0.22.0, controller-runtime v0.25.0, Kubernetes 1.37 support"*.
- controller-runtime **v0.25.1**, released 2026-09-14 — latest, two bugfixes (priority-queue `logState` data race; subresource create read-your-writes).

But they are four days apart in opposite directions, so `kubebuilder init` will scaffold a `go.mod` at v0.25.**0** and the first `go mod tidy` after it will not bump to v0.25.1 on its own. Worth one line in E3's first story so the spine's pin is not silently contradicted by the generated scaffold.

---

### F-7 — LOW — The `.wslconfig` recipe is real but incomplete, and is not risk-free

`kernelCommandLine = cgroup_no_v1=all` in `[wsl2]` of `%USERPROFILE%\.wslconfig` is a genuine, working setting — `cgroup_no_v1=<controllers|all>` is an upstream Linux boot parameter available since kernel 5.0, and WSL2's `kernelCommandLine` does append to the guest command line. Confirmed.

Two caveats the spine does not carry:

1. Community guidance consistently pairs it with `systemd.unified_cgroup_hierarchy=1` (`kernelCommandLine = cgroup_no_v1=all systemd.unified_cgroup_hierarchy=1`). With systemd enabled in WSL, `cgroup_no_v1=all` alone can leave a hybrid mount rather than a clean unified root — which is plausibly why F-1's check shows what it shows.
2. There is a known WSL failure mode (microsoft/WSL#9120) where adding `memory` to `cgroup_no_v1` with systemd enabled produces a catastrophic-failure boot. The setting is real; it is not a guaranteed-safe one-liner, and it needs `wsl --shutdown` plus verification afterwards.

---

### F-8 — LOW — Helm 4's `crds/` ordering is *probably* unchanged, but was not confirmable against a Helm 4 source

Two of three Helm claims are solidly verified:

- **Helm v4.3.0 is current** — released 2026-09-09, matches this host exactly (`v4.3.0`, GoVersion go1.27.1, KubeClientVersion v1.37). Next patch 4.3.1 on 2026-10-14. Confirmed.
- **Chart `apiVersion: v2` works unchanged under Helm 4, and v3 is experimental and off by default** — confirmed from the Helm 4 Overview (*"Charts v3 is in early development and available for experimentation. v2 charts continue to work unchanged"*; opt-in via `HELM_EXPERIMENTAL_CHART_V3` + `--chart-api-version=v3`) and from **HIP-0020** (*"the chart will be an opt-in feature"*, *"existing charts will be preserved"*, GA only after v4.0.0). AD-8's `apiVersion: v2` choice is correct and well-founded.
- **`crds/` installs before templates** — this is the one that could not be positively confirmed for Helm 4. The canonical page (helm.sh/docs/topics/charts) still states the behaviour (*"upload the CRDs, pause until the CRDs are made available by the API server, and then start the template engine"*) but carries a banner: *"This page has not yet been updated for Helm 4. Some of the content might be inaccurate or not applicable to Helm 4."* Neither the Helm 4 Overview nor the Helm 4 changelog addresses CRD handling, and pre-4.0 alpha notes flagged CRD handling as an area being refined.

The behaviour almost certainly holds, but AD-8 asserts it under a "Verified against live sources" heading on the strength of a page explicitly disclaimed for Helm 4. Verify empirically when the chart is built (`helm install --dry-run` ordering, or `--skip-crds` behaviour), and cite that instead.

---

### F-9 — LOW — Postgres 15 is two majors behind current, with no stated reason

`Postgres | 15` is a live, supported choice — PostgreSQL 15 has security support until **2027-11-11**. But 18 is the current stable series and 19 GA is imminent (Beta 3 shipped 2026-08-13). Every other row in the table is pinned to the newest release; this one is not, and the spine gives no rationale. Since `compose.yaml` and the k8s manifests already use `postgres:15` and the schema is seeded via initdb hooks, this is most likely inherited rather than chosen. One clause ("15 retained from the existing compose/init-hook setup; no feature depends on a later major") would make it a decision instead of a leftover.

---

## 2. Rows verified correct

Everything below was checked against a live upstream source today and is accurate as stated.

| Stack row | Verified value | Source |
| --- | --- | --- |
| Rust 1.98.1 is current | 1.98.1, 2026-09-01 | blog.rust-lang.org, releases.rs |
| Rust 1.97.1 exists | yes; host runs `rustc 1.97.1 (8bab26f4f 2026-07-14)` | releases.rs + host |
| tonic 0.14.6 | 0.14.6, 2026-05-07 | crates.io API |
| tonic-prost 0.14.6 | 0.14.6, 2026-05-07 | crates.io API |
| tonic-health 0.14.6 | 0.14.6, 2026-05-07 | crates.io API |
| sqlx 0.9.0 | 0.9.0, 2026-05-21 — latest; repo `Cargo.toml` and lock both at 0.9.0 | crates.io API + repo |
| tracing 0.1 | latest 0.1.44; repo lock 0.1.44 | crates.io API + repo |
| Go 1.27.1 | released 2026-09-01 (1.27 GA August 2026) | go.dev |
| Go >= 1.26 required by controller-runtime | **true** — controller-runtime v0.25.1 `go.mod` has `go 1.26.0` | raw.githubusercontent v0.25.1 go.mod |
| controller-runtime v0.25.1 | latest, 2026-09-14 | GitHub Releases API |
| controller-runtime pins k8s.io/api v0.37.0 | **true** — `k8s.io/api`, `k8s.io/apimachinery`, `k8s.io/client-go` all v0.37.0 | v0.25.1 go.mod |
| kubebuilder v4.16.0 | latest, 2026-09-10 | GitHub Releases API |
| k8s.io/api · client-go v0.37.0 | correct and consistent with controller-runtime | v0.25.1 go.mod |
| grpc-go v1.84.0 | latest, 2026-09-17 | GitHub Releases API |
| google.golang.org/protobuf v1.36.12 | latest, 2026-08-10 | GitHub Releases API |
| testcontainers-go v0.44.0 | latest, 2026-08-07 | GitHub Releases API |
| kind v0.33.0 | latest, 2026-08-26; host has `kind v0.33.0 go1.27.0 linux/amd64` | GitHub Releases API + host |
| kind v0.33.0 ships prebuilt kindest/node v1.37.0 | **true**, and it is the default; digest `sha256:a1ed56cf…580ae5` | kind v0.33.0 release body |
| kubectl v1.37.0 | Kubernetes v1.37.0 released 2026-08-26; host has `Client Version: v1.37.0` | GitHub Releases API + host |
| Helm v4.3.0 | latest, 2026-09-09; host has v4.3.0 | GitHub Releases API + host |
| chart `apiVersion: v2` under Helm 4 | supported unchanged; v3 experimental and opt-in | helm.sh/docs/overview, HIP-0020 |
| Postgres 15 | real and supported to 2027-11-11 (see F-9) | postgres.org versioning policy |

Every named technology still exists, is actively maintained, and fits its stated role. Nothing in the stack is abandoned, renamed, or superseded by a differently-named successor. Notably, "pinned by digest" in AD-11 is not hand-waving — kind's own v0.33.0 release body pushes digest pinning explicitly, so the decision matches upstream guidance even though the specific image cannot run here (F-1).

---

## 3. Repo-versus-spine reconciliation

| Spine claim | Repo reality | Status |
| --- | --- | --- |
| sqlx 0.9.0 | `Cargo.toml`: `sqlx = { version = "0.9.0", … }`; lock 0.9.0 | agrees |
| tonic/tonic-prost/tonic-health 0.14.6 | `Cargo.toml` caret `"0.14"`; lock 0.14.6 | agrees via lock |
| prost 0.14.6 | lock **0.14.4** | **spine wrong (F-2)** |
| tracing-subscriber 0.3 | **not a dependency** | **spine wrong (F-5)** |
| Rust toolchain 1.97.1 | no `rust-toolchain*` file exists | **unpinned (F-4)** |
| `rust:1.97-slim` builder | `rust:1.97-slim-bookworm@sha256:2775a09d…` | near-miss (F-4) |
| distroless nonroot runtime, uid 65532 | Dockerfile: `gcr.io/distroless/cc-debian12:nonroot@sha256:9dac0a79…`, `USER nonroot:nonroot` | agrees with AD-11's posture |
| `SQLX_OFFLINE` build with no DB | Dockerfile sets `ENV SQLX_OFFLINE=true`; `.sqlx/` is committed | agrees |
| node image v1.37.0 pinned by digest | `kind/cluster.yaml`: `kindest/node:v1.33.1`, **by tag** | **contradicts (F-1, F-3)** |
| `make deploy` runs `helm upgrade --install` | Makefile: `kubectl apply -f k8s/` | AD-8 is forward-looking; fine, but the spine does not say so |
| Postgres 15 | `compose.yaml`: `image: postgres:15` | agrees |
| "Go is not installed on this host" | `command -v go` → **not installed**. Correct. | agrees — a genuine reality check, and the document's best one |

**Additional host gaps not mentioned in the spine** (surfaced while reality-checking): `protoc` is **not installed**, and `build.rs` calls `tonic_prost_build::compile_protos`, which requires `protoc` on `PATH`. `cargo build`, `make check` and `make run` therefore fail on a clean checkout of this host today; only the Docker build works, because the Dockerfile installs `protobuf-compiler` itself. `sqlx-cli` is also **not installed**, so `make migrate` and `make prepare` fail — yet the Stack table asserts `sqlx + sqlx-cli 0.9.0 (must match)` as if both were present and verified. AD-4's *"`go build` and `go test` must never require `protoc`"* is a good rule; the Rust side has the exact problem AD-4 forbids on the Go side, and the spine does not acknowledge it.

---

## 4. Recommended edits

1. **AD-11 (blocking):** re-verify the host cgroup hierarchy before the next revision. Until `cat /sys/fs/cgroup/cgroup.controllers` returns content, do not claim the unified hierarchy or retire the node-image pin. State the fallback explicitly: `kindest/node:v1.34.11@sha256:44e222ee…` is the newest kind v0.33.0 prebuilt image that boots on cgroup v1, and `failCgroupV1: false` via `kubeadmConfigPatches` is the alternative that keeps the v1.37.0 pin.
2. **Stack table:** split `prost` out at **0.14.4**.
3. **AD-11 / `kind/cluster.yaml`:** change "1.36 onward" to **"1.35 onward"** in both places, citing KEP-5573.
4. **Stack table:** either add `tracing-subscriber = "0.3"` to `Cargo.toml` or mark the row as an E1 deliverable rather than a verified fact.
5. **Stack table:** say `rust:1.97-slim-bookworm`, and either add a `rust-toolchain.toml` pinning 1.97.1 or drop the word "toolchain".
6. **AD-8:** replace the `crds/`-ordering assertion with one verified against Helm 4 specifically, or mark it *to be confirmed when the chart lands* — the source page is disclaimed for Helm 4.
7. **E3 first story:** note that kubebuilder v4.16.0 scaffolds controller-runtime v0.25.0 and needs an explicit bump to v0.25.1.
8. **Stack table / Deferred:** record that `protoc` and `sqlx-cli` are absent from this host alongside the existing "Go is not installed" note — same class of fact, same blocking effect on a clean build.
9. **Header:** "Verified against live sources on 2026-09-19" is *earned* for the version rows and should stay. Consider narrowing it to the Stack table, so it does not appear to vouch for the environment claims in AD-11, which were not verified.

---

## 5. Sources

- [Announcing Rust 1.98.1 — Rust Blog](https://blog.rust-lang.org/2026/09/03/Rust-1.98.1/)
- [Rust Versions — releases.rs](https://releases.rs/)
- [crates.io API — tonic](https://crates.io/api/v1/crates/tonic) · [prost](https://crates.io/api/v1/crates/prost) · [tonic-prost](https://crates.io/api/v1/crates/tonic-prost) · [tonic-health](https://crates.io/api/v1/crates/tonic-health) · [sqlx](https://crates.io/api/v1/crates/sqlx) · [tracing-subscriber](https://crates.io/api/v1/crates/tracing-subscriber)
- [Go 1.27 Release Notes](https://go.dev/doc/go1.27) · [Go Release History](https://go.dev/doc/devel/release)
- [controller-runtime v0.25.1 go.mod](https://raw.githubusercontent.com/kubernetes-sigs/controller-runtime/v0.25.1/go.mod) · [controller-runtime releases](https://github.com/kubernetes-sigs/controller-runtime/releases/latest)
- [kubebuilder releases](https://github.com/kubernetes-sigs/kubebuilder/releases/latest)
- [grpc-go releases](https://github.com/grpc/grpc-go/releases/latest) · [protobuf-go releases](https://github.com/protocolbuffers/protobuf-go/releases/latest) · [testcontainers-go releases](https://github.com/testcontainers/testcontainers-go/releases/latest)
- [kind v0.33.0 release](https://github.com/kubernetes-sigs/kind/releases/latest) · [Kubernetes v1.37.0 release](https://github.com/kubernetes/kubernetes/releases/latest)
- [Helm releases](https://github.com/helm/helm/releases/latest) · [Helm 4 Overview](https://helm.sh/docs/overview/) · [Helm Charts topic](https://helm.sh/docs/topics/charts/) · [HIP-0020 Charts v3 Enablement](https://helm.sh/community/hips/hip-0020/)
- [KEP-5573 Remove cgroup v1 support](https://www.kubernetes.dev/resources/keps/5573/) · [SUSE KB: Nodes with cgroup v1 fail to start by default in Kubernetes v1.35+](https://support.scc.suse.com/s/kb/Nodes-with-cgroup-v1-fail-to-start-by-default-in-Kubernetes-v1-35)
- [microsoft/WSL#9120 — cgroup_no_v1 with systemd](https://github.com/microsoft/WSL/issues/9120) · [spurin/wsl-cgroupsv2](https://github.com/spurin/wsl-cgroupsv2)
- [PostgreSQL Versioning Policy](https://www.postgresql.org/support/versioning/)
- Host reality checks: `mount`, `/proc/cmdline`, `/sys/fs/cgroup/cgroup.controllers`, `rustc --version`, `kubectl version`, `kind version`, `helm version`, `command -v go protoc sqlx`
