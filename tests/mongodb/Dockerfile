FROM mongo:8.0
COPY entrypoint.sh healthcheck.sh /usr/local/bin/
COPY seed.js /seed/seed.js
RUN chmod +x /usr/local/bin/entrypoint.sh /usr/local/bin/healthcheck.sh
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
