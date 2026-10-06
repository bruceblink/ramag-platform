FROM debian:12-slim

RUN apt-get update \
    && apt-get install --no-install-recommends --yes openssh-server python3 \
    && rm -rf /var/lib/apt/lists/*

# UsePAM=no rejects a locked account even for public-key authentication.
# The dedicated test account has no password; sshd disables both password
# and keyboard-interactive authentication and accepts the generated key only.
RUN useradd --create-home --shell /bin/bash ramag \
    && passwd --delete ramag \
    && mkdir --parents /run/sshd /home/ramag/ramag-fixture \
    && chown --recursive ramag:ramag /home/ramag

COPY sshd_config /etc/ssh/sshd_config
COPY entrypoint.sh /usr/local/bin/ramag-ssh-entrypoint
RUN chmod 0755 /usr/local/bin/ramag-ssh-entrypoint

ENTRYPOINT ["/usr/local/bin/ramag-ssh-entrypoint"]
