#!/usr/bin/env python3
"""
Shelly Bridge — translates Codex CLI / Gemini CLI command hooks
into HTTP requests to the Shelly overlay server.

Usage: python3 shelly-bridge.py <agent> <endpoint>
  agent:    codex-cli | gemini-cli
  endpoint: permission | notification | stop
"""

import sys
import json
import urllib.request
import urllib.error

SHELLY_URL = "http://localhost:21517"


def main():
    if len(sys.argv) < 3:
        sys.exit(0)

    agent = sys.argv[1]
    endpoint = sys.argv[2]

    try:
        raw = sys.stdin.read()
        if not raw.strip():
            sys.exit(0)
        data = json.loads(raw)
    except (json.JSONDecodeError, IOError):
        sys.exit(0)

    data["agent"] = agent

    # Normalize fields per agent
    if agent == "gemini-cli" and endpoint == "permission":
        # Gemini BeforeTool uses tool_name + tool_input, same as Shelly
        pass
    elif agent == "codex-cli" and endpoint == "permission":
        # Codex PreToolUse uses tool_name + tool_input, same as Shelly
        pass

    # Map endpoint to Shelly HTTP path
    path_map = {
        "permission": "/hooks/permission",
        "notification": "/hooks/notification",
        "stop": "/hooks/stop",
    }
    path = path_map.get(endpoint)
    if not path:
        sys.exit(0)

    # POST to Shelly
    url = SHELLY_URL + path
    try:
        req = urllib.request.Request(
            url,
            data=json.dumps(data).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=130) as resp:
            response = json.loads(resp.read().decode("utf-8"))
    except (urllib.error.URLError, IOError, json.JSONDecodeError):
        # Shelly not running — pass through (allow)
        sys.exit(0)

    # Transform response back to agent-expected format
    if endpoint == "permission":
        behavior = "allow"
        hook_output = response.get("hookSpecificOutput", {})
        decision = hook_output.get("decision", {})
        if isinstance(decision, dict):
            behavior = decision.get("behavior", "allow")

        if agent == "codex-cli":
            if behavior == "deny":
                print(json.dumps({
                    "hookSpecificOutput": {
                        "hookEventName": "PreToolUse",
                        "permissionDecision": "deny",
                        "permissionDecisionReason": "Denied by Shelly",
                    }
                }))
            # codex allow = no output, exit 0
        elif agent == "gemini-cli":
            # Gemini requires explicit JSON decision on stdout for both allow and deny.
            if behavior == "deny":
                print(json.dumps({
                    "decision": "deny",
                    "reason": "Denied by Shelly",
                }))
            else:
                print(json.dumps({"decision": "allow"}))

    # notification and stop don't need output transformation


if __name__ == "__main__":
    main()
