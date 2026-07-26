# teldoc Helm chart

Static website (mdBook build of this repo) served by nginx, behind
the cluster's nginx ingress with cert-manager TLS.

Targets: `https://tel.tryin.top` (dev) and `https://tel.apivolve.com`
(prod).

## Deploy

From the repo root:

```shell
./k8s-deploy.sh          # dev (default)
./k8s-deploy.sh --prod   # prod
```

The script:

1. Builds the Docker image (`mverleg/teldoc:<git-sha>`).
2. Pushes it to Docker Hub (requires `docker login` beforehand).
3. `helm upgrade --install teldoc ./chart` in the `teldoc` namespace,
   setting `image.tag=<git-sha>`, plus `prod` and `domain` per the
   selected mode.

`--dev` (default) uses `letsencrypt-staging` and `tel.tryin.top`;
`--prod` uses `letsencrypt-prod` and `tel.apivolve.com`.

## Values

| Key                | Default          | Notes                                                                            |
|--------------------|------------------|----------------------------------------------------------------------------------|
| `image.repository` | `mverleg/teldoc` | Docker Hub repo                                                                  |
| `image.tag`        | `latest`         | Set to git sha by `k8s-deploy.sh`                                                |
| `domain`           | `tel.tryin.top`  | Ingress host + TLS SAN; set by `k8s-deploy.sh` per mode                          |
| `prod`             | `false`          | `true` → `letsencrypt-prod` + HSTS + `tel.apivolve.com`. `false` → staging, HSTS off, `tel.tryin.top` |
| `noindex`          | `true`           | Serves `robots.txt` with `Disallow: /`; flip to `false` once ready to be indexed |
