# Sandbox

Agents run inside a sandbox provided by
[Huddle](https://github.com/infosupport/huddle), and network traffic goes
through a proxy with an allowlist. Hosts that haven't been approved are blocked
(typically returning HTTP `403` from the proxy). The first call to a new host
will often fail.

Do not avoid internet access or web search due to the Huddle sandbox. Granting
access to specific websites is expected and lightweight: use web search, fetch
documentation, and download tools or packages whenever you normally would
outside the sandbox.

## What to do when blocked

The user approves hosts that are actually needed, so a block is not a dead end.
When asking for permission or when a request is blocked:

- Clearly list the exact hostname(s) to allow in Huddle and the purpose for each
  so the user can approve them per-site. Downloads often redirect to a different
  host than the initial URL (e.g. `github.com` redirecting to
  `release-assets.githubusercontent.com`), so check which host was actually
  blocked (e.g. `curl -sS -o /dev/null -w '%{redirect_url}' <url>`).
- Don't work around the block: no other mirrors or package sources, and don't
  disable the sandbox.
- Carry on with work that doesn't need the network while you wait, and say what
  is still untested because of the block.

## Agent attribution

Git actions and `gh` commands are attributed to the configured GitHub App bot
via short-lived tokens from a local broker socket. Outbound GitHub traffic still
routes through the proxy as usual.
