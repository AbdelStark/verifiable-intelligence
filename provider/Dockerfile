# syntax=docker/dockerfile:1.7
#
# Provider image skeleton for the CommitLLM-backed live provider path.
# Weights, healthz, entrypoint wiring, and deployment recipes land in the
# follow-up provider issues; this file fixes the reproducible base/build shape.

ARG CUDA_DEVEL_IMAGE=nvidia/cuda:12.4.1-devel-ubuntu22.04@sha256:da6791294b0b04d7e65d87b7451d6f2390b4d36225ab0701ee7dfec5769829f5
ARG CUDA_RUNTIME_IMAGE=nvidia/cuda:12.4.1-runtime-ubuntu22.04@sha256:517da2300c184c9999ec203c2665244bdebd3578d12fcc7065e83667932643d9
ARG COMMITLLM_SHA=25541e83347655e44ad6e84eb901e1e7ae392a66
ARG COMMITLLM_REPO=https://github.com/lambdaclass/CommitLLM.git

FROM ${CUDA_DEVEL_IMAGE} AS provider-builder

ARG COMMITLLM_REPO
ARG COMMITLLM_SHA
ENV DEBIAN_FRONTEND=noninteractive

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt/lists,sharing=locked \
    apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        git \
        python3 \
        python3-pip \
        python3-venv \
    && rm -rf /var/lib/apt/lists/*

RUN git clone --filter=blob:none "${COMMITLLM_REPO}" /opt/commitllm \
    && cd /opt/commitllm \
    && git checkout "${COMMITLLM_SHA}" \
    && test "$(git rev-parse HEAD)" = "${COMMITLLM_SHA}"

RUN python3 -m venv /opt/provider-venv \
    && /opt/provider-venv/bin/python -m pip install --upgrade pip setuptools wheel

RUN --mount=type=cache,target=/root/.cache/pip,sharing=locked \
    mkdir -p /opt/wheelhouse \
    && /opt/provider-venv/bin/python -m pip wheel \
        --no-deps \
        --wheel-dir /opt/wheelhouse \
        /opt/commitllm/sidecar

FROM ${CUDA_RUNTIME_IMAGE} AS provider-runtime

ARG COMMITLLM_SHA
ENV DEBIAN_FRONTEND=noninteractive
ENV COMMITLLM_SHA=${COMMITLLM_SHA} \
    COMMITLLM_SHORT=25541e83 \
    MODEL_ID=llama-3.1-8b-w8a8 \
    MODEL_DIR=/weights \
    PATH=/opt/provider-venv/bin:${PATH} \
    VI_BIND_ADDR=0.0.0.0:8000 \
    VI_LOG_LEVEL=info \
    VLLM_PLUGINS=verilm_capture

LABEL org.opencontainers.image.source="https://github.com/AbdelStark/verifiable-intelligence" \
      org.opencontainers.image.description="Verifiable Intelligence CommitLLM provider skeleton" \
      org.opencontainers.image.revision="${COMMITLLM_SHA}"

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt/lists,sharing=locked \
    apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        python3 \
        python3-pip \
        python3-venv \
    && rm -rf /var/lib/apt/lists/*

COPY --from=provider-builder /opt/wheelhouse /opt/wheelhouse
COPY provider/entrypoint.sh /entrypoint.sh
COPY provider/healthz.py /healthz.py

RUN python3 -m venv /opt/provider-venv \
    && /opt/provider-venv/bin/python -m pip install \
        --no-index \
        --no-deps \
        --find-links=/opt/wheelhouse \
        verilm \
    && rm -rf /root/.cache/pip

RUN chmod +x /entrypoint.sh /healthz.py

WORKDIR /srv/provider
EXPOSE 8000

ENTRYPOINT ["/entrypoint.sh"]
