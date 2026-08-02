# @rambledesk/pi

Pi-native RambleDesk package. It registers:

- `request_ramble_feedback`: creates a RambleDesk feedback request through the
  local JSON API and waits inside the same Pi tool call until the human submits
  or cancels.
- `get_ramble_feedback`: reads a request by `request_id` for recovery or
  diagnostics.

Install from a source checkout:

```sh
pi install ./packages/pi-rambledesk
```

After package publication, the intended install form is:

```sh
pi install npm:@rambledesk/pi
```

The package does not use MCP. It talks to RambleDesk's authenticated loopback
JSON API:

- default API URL: `http://127.0.0.1:37642/api`
- override API URL: `RAMBLEDESK_LOCAL_API_URL`
- override port: `RAMBLEDESK_LOCAL_SERVER_PORT`
- token: `RAMBLEDESK_LOCAL_SERVER_TOKEN`, or the RambleDesk local-server token
  file (`RAMBLEDESK_LOCAL_SERVER_TOKEN_FILE` can override the path)

The tool waits by holding the Pi tool execution, not by asking RambleDesk to
continue Pi later. Once it returns `completed`, read
`details.feedback_package.markdown` and `details.feedback_package.attachment_paths`.
