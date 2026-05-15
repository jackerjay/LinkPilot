# Security Policy

## Supported versions

Security fixes target the latest released version of LinkPilot.

## Reporting a vulnerability

Please do not open a public issue for security vulnerabilities.

Report security issues through GitHub's private vulnerability reporting for this
repository. If that is unavailable, contact the maintainer privately and include:

- Affected version or commit
- Operating system and browser/app involved
- Reproduction steps
- Impact assessment
- Any known workaround

We aim to acknowledge valid reports within 7 days.

## Scope

In scope:

- URL routing behavior that can execute an unexpected browser/app action
- Local IPC handling
- Config parsing and file watching
- macOS default-browser and login-item integration

Out of scope:

- Social engineering
- Physical device compromise
- Denial-of-service reports without a security impact
- Vulnerabilities in unsupported operating systems or unreleased stubs
