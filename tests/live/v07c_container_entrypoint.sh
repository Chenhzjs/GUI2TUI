#!/usr/bin/env bash
set -euo pipefail

authorized_source=/run/gui2tui-test/authorized_key.pub
authorized_dir=/home/gui2tui/.ssh
authorized_keys=$authorized_dir/authorized_keys

[[ -f $authorized_source && ! -L $authorized_source ]] || {
    echo 'error: the disposable SSH public key was not mounted' >&2
    exit 1
}
install -d -o gui2tui -g gui2tui -m 0700 -- "$authorized_dir"
install -o gui2tui -g gui2tui -m 0600 -- "$authorized_source" "$authorized_keys"
ssh-keygen -A >/dev/null
exec /usr/sbin/sshd -D -e
