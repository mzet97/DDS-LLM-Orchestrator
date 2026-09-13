# Imagem do runner dedicado à tese (bootstrap; digest registrado no build).
#
# Contém act_runner + toolchain homologado (Rust 1.95) + nativos + Node
# (Actions JS como checkout). Sem Docker socket, sem credenciais, sem
# kubeconfig, sem SSH das VMs: tudo isso é proibido também no manifesto.
# Jobs executam no próprio contêiner (label tese-rust:host).
#
# Build inicial (bootstrap, ferramenta já existente no .51):
#   docker build -t tese-runner:0.1.0 ci/tese-runner
# Digest: anotar via `docker inspect` e fixar no manifesto antes de ativar.

FROM ubuntu:22.04

ARG ACT_RUNNER_VERSION=0.2.12
ARG ACT_RUNNER_SHA256=0b6d1ca5487e737bc67ecb440997dff412dd63d65b291eddcb69aec9cb61ebbf
ARG RUST_TOOLCHAIN=1.95.0
ARG NODE_MAJOR=24

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:/usr/local/bin:/usr/bin:/bin

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates curl xz-utils git cmake gcc g++ make libc6-dev pkg-config \
      libssl-dev gnupg passwd \
 && curl -fsSL https://deb.nodesource.com/setup_${NODE_MAJOR}.x | bash - \
 && apt-get install -y --no-install-recommends nodejs \
 && rm -rf /var/lib/apt/lists

RUN curl -fsSL -o /tmp/act_runner.xz \
      "https://dl.gitea.com/act_runner/${ACT_RUNNER_VERSION}/act_runner-${ACT_RUNNER_VERSION}-linux-amd64.xz" \
 && echo "${ACT_RUNNER_SHA256}  /tmp/act_runner.xz" | sha256sum -c - \
 && unxz /tmp/act_runner.xz \
 && mv /tmp/act_runner /usr/local/bin/act_runner \
 && chmod +x /usr/local/bin/act_runner

RUN curl -fsSL https://sh.rustup.rs -o /tmp/rustup.sh \
 && sh /tmp/rustup.sh -y --profile minimal --default-toolchain "${RUST_TOOLCHAIN}" \
 && rm /tmp/rustup.sh \
 && rustup component add rustfmt clippy \
 && rustc --version && cargo --version && cargo fmt --version && cargo clippy --version \
 && cmake --version | head -1

# Toolchain fixada por env: os proxies rustup/cargo não tentam sincronizar
# RUSTUP_HOME (root) a cada invocação — erro observado na 0.2.0 sob uid 1001.
ENV RUSTUP_TOOLCHAIN=${RUST_TOOLCHAIN}-x86_64-unknown-linux-gnu

# Registry/config do cargo graváveis pelo UID 1001 do job (cargo fetch,
# caches de registry) — sem exigir apt-get/sudo/--user 0 durante o job.
RUN chown -R 1001:1001 /usr/local/cargo /usr/local/rustup

# Base mínima sem `useradd`: entrada passwd explícita (uid 1001, sem login).
RUN echo 'runner:x:1001:1001:CI:/data:/bin/sh' >> /etc/passwd \
 && mkdir -p /data && chown 1001:1001 /data
COPY --chown=1001:1001 entrypoint.sh /usr/local/bin/entrypoint.sh
COPY cyclonedds.xml /etc/cyclonedds.xml
RUN chmod +x /usr/local/bin/entrypoint.sh
# Descoberta DDS determinística no namespace do contêiner (ver comentário
# no XML): sem isto, readers perdem writers de força menor em tópicos
# keyless+Exclusive (claim.rs via 21/100; com config, 100/100).
ENV CYCLONEDDS_URI=file:///etc/cyclonedds.xml
ENV HOME=/data
USER 1001
WORKDIR /data
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
