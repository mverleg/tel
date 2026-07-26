#!/usr/bin/env -S bash -eEu -o pipefail
#
# Build the teldoc image, push it to Docker Hub, and
# helm upgrade --install the chart.
#
# Usage: ./k8s-deploy.sh [--prod | --dev]
#
#   --dev   (default) letsencrypt-staging cert, domain tel.tryin.top,
#                     namespace + release teldoc-dev
#   --prod            letsencrypt-prod cert,    domain tel.apivolve.com,
#                     namespace + release teldoc-prod
#
# Requires: kubectl context pointing at the tryin.top cluster, and
# `docker login` already done for the mverleg Docker Hub account.

BASE="teldoc"
REPOSITORY="mverleg/teldoc"

PROD=false
while [ $# -gt 0 ]; do
    case "$1" in
        --prod) PROD=true ;;
        --dev)  PROD=false ;;
        *) echo "unknown argument: $1 (expected --prod or --dev)" >&2; exit 2 ;;
    esac
    shift
done

if [ "${PROD}" = true ]; then
    ENV=prod
    DOMAIN="tel.apivolve.com"
    TEL_DEV=""          # public book: no TIPs / draft proposals
else
    ENV=dev
    DOMAIN="tel.tryin.top"
    TEL_DEV="1"         # dev book: include the TIPs (see inject-tips.py)
fi

NAMESPACE="${BASE}-${ENV}"
RELEASE="${BASE}-${ENV}"

cd "$(dirname "$(realpath "$0")")"

TAG="$(git rev-parse --short HEAD 2>/dev/null || date +%Y%m%d%H%M%S)"
if [ -n "$(git status --porcelain . 2>/dev/null || true)" ]; then
    TAG="${TAG}-dirty-$(date +%Y%m%d%H%M%S)"
fi

IMAGE="${REPOSITORY}:${TAG}"

echo "=== estimated print size ==="
TEL_DEV="${TEL_DEV}" python3 ./scripts/estimate-pages.py || \
    echo "(page estimate skipped)"

echo "=== building image (${IMAGE}, TEL_DEV='${TEL_DEV}') ==="
docker build --build-arg "TEL_DEV=${TEL_DEV}" -t "${IMAGE}" .

echo "=== pushing image ==="
docker push "${IMAGE}"

echo "=== helm upgrade (release=${RELEASE}, ns=${NAMESPACE}, tag=${TAG}, prod=${PROD}, domain=${DOMAIN}) ==="
helm upgrade --install "${RELEASE}" ./chart \
    --namespace "${NAMESPACE}" --create-namespace \
    --set "image.tag=${TAG}" \
    --set "prod=${PROD}" \
    --set "domain=${DOMAIN}"

echo "=== done ==="
kubectl -n "${NAMESPACE}" rollout status "deploy/${RELEASE}-deployment" --timeout=60s
kubectl -n "${NAMESPACE}" get pods,svc,ingress
