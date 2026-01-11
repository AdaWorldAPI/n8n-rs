FROM n8nio/n8n:latest

# Auth - use Railway env vars for secrets
ENV N8N_BASIC_AUTH_ACTIVE=true
# N8N_BASIC_AUTH_USER and N8N_BASIC_AUTH_PASSWORD from Railway env

# Webhook auth
ENV N8N_WEBHOOK_TUNNEL_URL=https://n8n.exo.red
ENV WEBHOOK_URL=https://n8n.exo.red

# Security
ENV N8N_SECURE_COOKIE=true
ENV N8N_ENCRYPTION_KEY=${N8N_ENCRYPTION_KEY}

# Disable public API unless authenticated  
ENV N8N_PUBLIC_API_DISABLED=false

# Host config
ENV N8N_HOST=0.0.0.0
ENV N8N_PORT=5678
ENV N8N_PROTOCOL=https
ENV GENERIC_TIMEZONE=Europe/Berlin

# Ada endpoints
ENV ADA_MCP_URL=https://mcp.exo.red
ENV ADA_POINT_URL=https://point.exo.red

WORKDIR /home/node

COPY workflows /home/node/workflows

EXPOSE 5678

CMD ["n8n", "start"]
