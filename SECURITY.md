# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Reporting a Vulnerability

If you discover a security vulnerability in seuil-rs, please report it responsibly:

1. **Do not** open a public GitHub issue
2. Email security@zuub.com with:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact assessment
3. We will acknowledge receipt within 48 hours
4. We will provide a fix or mitigation within 7 days for critical issues

## Security Design

seuil-rs is designed with security as a first-class concern:

- **`#![forbid(unsafe_code)]`** — no unsafe Rust anywhere in the crate
- **Resource limits** — configurable depth limits, time limits, and memory limits prevent denial-of-service via crafted expressions
- **No panics on any input** — all malformed expressions and data produce `Err`, never crash the process
- **Continuous fuzzing** — coverage-guided fuzzing runs nightly via GitHub Actions
- **VOPR campaigns** — 10,000+ seed verification campaigns run on every release
- **Chaos testing** — 9 fault injection categories tested on every CI run

## Scope

The following are considered security-relevant:

- Panics or crashes from any input (expression or JSON data)
- Memory exhaustion without hitting configured limits
- Infinite loops without hitting configured time limits
- Information leaks through error messages
- Any behavior that contradicts `#![forbid(unsafe_code)]`
