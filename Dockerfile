FROM docker.n8n.io/n8nio/n8n:latest

USER root

COPY workflows /home/node/workflows
RUN chown -R node:node /home/node/workflows

# Create persistent data dir
RUN mkdir -p /home/node/.n8n && chown -R node:node /home/node/.n8n

USER node

# Railway port
ENV N8N_PORT=8080
ENV N8N_HOST=0.0.0.0
ENV N8N_PROTOCOL=https
ENV GENERIC_TIMEZONE=Europe/Berlin

# Security
ENV N8N_DIAGNOSTICS_ENABLED=false
ENV N8N_VERSION_NOTIFICATIONS_ENABLED=false
ENV N8N_TEMPLATES_ENABLED=false
ENV N8N_PERSONALIZATION_ENABLED=false
ENV N8N_HIRING_BANNER_ENABLED=false
ENV N8N_RUNNERS_ENABLED=false

ENV N8N_EDITOR_BASE_URL=https://n8n.exo.red

# Use external Postgres instead of SQLite for persistence
# Set these in Railway:
# DB_TYPE=postgresdb
# DB_POSTGRESDB_HOST=...
# DB_POSTGRESDB_DATABASE=n8n
# DB_POSTGRESDB_USER=...
# DB_POSTGRESDB_PASSWORD=...

EXPOSE 8080
