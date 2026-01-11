FROM docker.n8n.io/n8nio/n8n:latest

USER root

# Copy workflows
COPY workflows /home/node/workflows
RUN chown -R node:node /home/node/workflows

USER node

# Non-sensitive config only
ENV N8N_HOST=0.0.0.0
ENV N8N_PORT=5678
ENV N8N_PROTOCOL=https
ENV GENERIC_TIMEZONE=Europe/Berlin

# Security hardening
ENV N8N_DIAGNOSTICS_ENABLED=false
ENV N8N_VERSION_NOTIFICATIONS_ENABLED=false
ENV N8N_TEMPLATES_ENABLED=false
ENV N8N_PERSONALIZATION_ENABLED=false
ENV N8N_HIRING_BANNER_ENABLED=false

# Completely disable task runners (no Python warning)
ENV N8N_RUNNERS_ENABLED=false
ENV N8N_RUNNERS_TASK_TIMEOUT=60
ENV N8N_RUNNERS_MODE=internal_childprocess
ENV N8N_RUNNERS_AUTO_SHUTDOWN_TIMEOUT=0

# Restrict editor access
ENV N8N_EDITOR_BASE_URL=https://n8n.exo.red

# Only expose main port
EXPOSE 5678
