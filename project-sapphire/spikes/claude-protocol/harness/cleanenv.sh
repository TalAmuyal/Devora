#!/bin/bash
# Run a command with a minimal env: drop this session's identity/telemetry vars, keep auth/proxy.
keep=(PATH HOME USER LANG TERM SHELL TMPDIR ANTHROPIC_BASE_URL HTTPS_PROXY HTTP_PROXY https_proxy http_proxy NO_PROXY no_proxy GLOBAL_AGENT_NO_PROXY NODE_EXTRA_CA_CERTS SSL_CERT_FILE REQUESTS_CA_BUNDLE CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST CLAUDE_CODE_ACCOUNT_UUID CLAUDE_CODE_ORGANIZATION_UUID CLAUDE_CODE_USER_EMAIL CLAUDE_CODE_PROXY_RESOLVES_HOSTS)
args=()
for k in "${keep[@]}"; do [[ -n "${!k+x}" ]] && args+=("$k=${!k}"); done
exec env -i "${args[@]}" "$@"
