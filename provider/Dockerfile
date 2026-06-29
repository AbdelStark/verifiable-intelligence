# syntax=docker/dockerfile:1
#
# Provider runtime image work lands in the provider issues. This stage records
# the CommitLLM checkout contract from RFC-0011 so Docker-based provider work
# cannot silently drift from commitllm.lock.

FROM alpine/git:2.49.1 AS commitllm-source

ARG COMMITLLM_SHA=25541e83347655e44ad6e84eb901e1e7ae392a66

RUN git clone https://github.com/lambdaclass/CommitLLM /opt/commitllm \
    && cd /opt/commitllm \
    && git checkout "${COMMITLLM_SHA}"
