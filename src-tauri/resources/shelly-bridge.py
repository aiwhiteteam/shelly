#!/usr/bin/env python3
"""
Shelly Bridge — translates Codex CLI / Gemini CLI command hooks
into HTTP requests to the Shelly overlay server.

Usage: python3 shelly-bridge.py <agent> <endpoint>
  agent:    codex-cli | gemini-cli
  endpoint: permission | notification | stop
"""

import os
import sys
import json
import urllib.request
import urllib.error

SHELLY_URL = "http://localhost:21517"


def post(url, data, timeout):
    req = urllib.request.Request(
        url,
        data=json.dumps(data).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


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
    # Override session_id with the agent process's PID so Shelly's
    # jump-to-terminal can resolve it back to a TTY for tab targeting.
    data["session_id"] = str(os.getppid())

    path_map = {
        "permission": "/hooks/permission",
        "notification": "/hooks/notification",
        "stop": "/hooks/stop",
    }
    path = path_map.get(endpoint)
    if not path:
        sys.exit(0)
    url = SHELLY_URL + path

    # Notification and stop are fire-and-forget (short hook timeouts in the
    # CLI configs, no response transformation needed).
    if endpoint in ("notification", "stop"):
        try:
            post(url, data, timeout=2)
        except (urllib.error.URLError, IOError, json.JSONDecodeError):
            pass
        sys.exit(0)

    # Permission: wait for the user's decision, transform per agent.
    try:
        response = post(url, data, timeout=130)
    except (urllib.error.URLError, IOError, json.JSONDecodeError):
        # Shelly not running — pass through (allow)
        if agent == "gemini-cli":
            print(json.dumps({"decision": "allow"}))
        sys.exit(0)

    behavior = "allow"
    hook_output = response.get("hookSpecificOutput", {})
    decision = hook_output.get("decision", {})
    if isinstance(decision, dict):
        behavior = decision.get("behavior", "allow")

    if agent in ("codex-cli", "cursor", "opencode"):
        # Codex, Cursor, OpenCode: deny = print hookSpecificOutput, allow = no output
        if behavior == "deny":
            print(json.dumps({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": "Denied by Shelly",
                }
            }))
    elif agent == "gemini-cli":
        # Gemini requires explicit JSON decision on stdout for both allow and deny.
        if behavior == "deny":
            print(json.dumps({
                "decision": "deny",
                "reason": "Denied by Shelly",
            }))
        else:
            print(json.dumps({"decision": "allow"}))


if __name__ == "__main__":
    main()
