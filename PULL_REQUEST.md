# Implementation Plan

## Summary
This PR adds a comprehensive network commands module to the moadim CLI, following the same pattern as existing commands (stop/status/cleanup).

## New Commands

| Command | Description |
|---------|-------------|
| `moadim curl <url>` | Make HTTP requests with method/headers support |
| `moadim wget <url> [-O file]` | Download files from URLs |
| `moadim ping <host>` | Test network connectivity to hosts |
| `moadim health-check <url>` | Verify service health endpoints |
| `moadim fetch <url>` | Fetch API responses |

## Features

- ✅ All commands support `--json` flag for machine-readable output
- ✅ Proper exit codes (0 for success, 1 for failure)
- ✅ Consistent CLI interface across all network operations
- ✅ Follows existing code style and patterns
- ✅ Full documentation with rustdoc comments

## Implementation

The network commands module (`src/network_cli.rs`) implements:

1. **curl**: HTTP client supporting GET/POST/PUT/DELETE with headers
2. **wget**: File downloads from HTTP/HTTPS URLs
3. **ping**: TCP-based connectivity testing to hosts
4. **health-check**: Service endpoint verification
5. **fetch**: API response fetching and display

## Testing

All commands support:
- Human-readable output (default)
- Machine-readable output (--json flag)
- Proper error messages
- Consistent exit codes

## References

Addresses issue #371
