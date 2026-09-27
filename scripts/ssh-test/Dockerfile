FROM debian:12-slim

RUN apt-get update \
    && apt-get install --no-install-recommends --yes openssh-server \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /bin/bash ramag \
    && mkdir --parents /run/sshd /home/ramag/ramag-fixture \
    && chown --recursive ramag:ramag /home/ramag

COPY sshd_config /etc/ssh/sshd_config
COPY entrypoint.sh /usr/local/bin/ramag-ssh-entrypoint
RUN chmod 0755 /usr/local/bin/ramag-ssh-entrypoint

ENTRYPOINT ["/usr/local/bin/ramag-ssh-entrypoint"]
