# Security Notes

The control panel treats terminal access as sensitive server access.

Key constraints:

- use auth cookies
- keep terminal/file access behind the API
- restrict file operations to allowlisted paths
- avoid shell injection in service controls
- do not expose the agent directly to the internet
