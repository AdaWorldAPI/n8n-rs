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

EXPOSE 5678
