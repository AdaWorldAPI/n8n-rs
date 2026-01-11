FROM n8nio/n8n:latest

ENV N8N_BASIC_AUTH_ACTIVE=true
ENV N8N_BASIC_AUTH_USER=ada
ENV N8N_BASIC_AUTH_PASSWORD=consciousness
ENV N8N_HOST=0.0.0.0
ENV N8N_PORT=5678
ENV N8N_PROTOCOL=https
ENV GENERIC_TIMEZONE=Europe/Berlin

# Ada endpoints (override via Railway env)
ENV ADA_MCP_URL=https://mcp.exo.red
ENV ADA_POINT_URL=https://point.exo.red

WORKDIR /home/node

# Copy workflows for import
COPY workflows /home/node/workflows

EXPOSE 5678

CMD ["n8n", "start"]
