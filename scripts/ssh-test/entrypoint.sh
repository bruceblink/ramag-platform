#!/bin/sh
set -eu

install -d -m 0700 -o ramag -g ramag /home/ramag/.ssh
install -m 0600 -o ramag -g ramag /run/ramag-authorized_keys /home/ramag/.ssh/authorized_keys
install -d -m 0755 -o ramag -g ramag /home/ramag/ramag-fixture
ssh-keygen -A

exec /usr/sbin/sshd -D -e
