FROM alpine:3.20
RUN apk add --no-cache openssh-server \
    && for user in tunnel dropped; do \
        adduser -D -s /bin/sh "$user" \
        && echo "$user:$user" | chpasswd \
        && install -d -m 700 -o "$user" -g "$user" "/home/$user/.ssh"; \
    done
COPY sshd_config /etc/ssh/sshd_config
COPY ssh_host_ed25519_test /etc/ssh/ssh_host_ed25519_key
COPY ssh_host_ed25519_test.pub /etc/ssh/ssh_host_ed25519_key.pub
COPY ssh_host_ecdsa_test /etc/ssh/ssh_host_ecdsa_key
COPY ssh_host_ecdsa_test.pub /etc/ssh/ssh_host_ecdsa_key.pub
COPY tunnel_test_ed25519.pub /home/tunnel/.ssh/authorized_keys
COPY tunnel_test_ed25519.pub /home/dropped/.ssh/authorized_keys
RUN chmod 600 /etc/ssh/ssh_host_ed25519_key /etc/ssh/ssh_host_ecdsa_key \
    && for user in tunnel dropped; do \
        chown "$user:$user" "/home/$user/.ssh/authorized_keys" \
        && chmod 600 "/home/$user/.ssh/authorized_keys"; \
    done
EXPOSE 22
CMD ["/usr/sbin/sshd", "-D", "-e"]
