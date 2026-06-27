# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in conflux-engine, please report it responsibly.

**Do not open a public GitHub issue for security vulnerabilities.**

Instead, email the maintainers with:

1. A description of the vulnerability
2. Steps to reproduce
3. Potential impact assessment
4. Any suggested fix (optional)

We aim to acknowledge reports within **72 hours** and provide a remediation timeline within **14 days** for confirmed issues.

## Scope

The following are in scope:

- Remote code execution via subscription parsing or IPC
- Privilege escalation through the named pipe interface
- Credential leakage in logs, IPC responses, or crash dumps
- TLS verification bypass in subscription fetch
- Unsafe process spawning in the sing-box backend adapter

The following are generally out of scope:

- Denial of service via oversized subscription bodies (mitigate with configurable limits)
- Vulnerabilities in the sing-box binary itself (report to [sing-box](https://github.com/SagerNet/sing-box))
- Social engineering or physical access attacks
- Issues in downstream desktop VPN client applications

## Security Considerations

### Subscription URLs

Subscription URLs contain access tokens. Treat them as secrets:

- Do not log full URLs at info level or above
- Do not include real tokens in bug reports or test fixtures
- Use placeholder URLs like `https://example.com/sub/token` in documentation

### IPC Pipe

The named pipe accepts commands from authenticated local users. In v0.1, `FETCH` can trigger outbound network requests. Host the daemon elevated and restrict pipe ACL to authenticated users only.

### Credential Handling

Normalized profiles contain proxy credentials. IPC responses redact sensitive fields; full credentials are used only internally for backend config generation.

### Dependency Updates

Security-relevant dependencies (reqwest, rustls, tokio) are pinned in `Cargo.lock`. Release branches receive dependency patch updates.

## Safe Harbor

We support good-faith security research. Researchers who follow this policy will not be subject to legal action for authorized testing against their own installations.
