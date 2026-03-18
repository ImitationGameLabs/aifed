# aifed-daemon HTTP API Design

## Overview

- **Protocol**: HTTP over Unix Socket
- **Base URL**: `http://localhost` (over socket)
- **API Version**: `/api/v1`
- **Socket Path**: `~/.cache/aifed/<name>-<hash16>.sock`

## Architecture

Each daemon instance is bound to **exactly one workspace**. The daemon:
- Starts automatically when needed (by aifed CLI)
- Detects languages in the workspace root and starts relevant LSP servers
- Shuts down after 30 minutes of inactivity
- Socket path is deterministic based on workspace canonical path

### Socket Path Generation

```
~/.cache/aifed/<name>-<hash16>.sock
```

- `name`: sanitized workspace directory name (alphanumeric, dash, underscore, max 32 chars)
- `hash16`: first 16 hex chars of xxh64 hash of canonical path

Example: `~/.cache/aifed/aifed-8f3a2b1c9d4e5f6a.sock`

## Response Format

### Success

```json
{
  "success": true,
  "data": { ... }
}
```

### Error

```json
{
  "success": false,
  "error": {
    "code": "LSP_ERROR",
    "message": "LSP server not running for language: rust"
  }
}
```

---

## Endpoints

### 1. Daemon Management

| Method | Endpoint         | Description                                |
| ------ | ---------------- | ------------------------------------------ |
| GET    | `/api/v1/health` | Health check                               |
| GET    | `/api/v1/status` | Daemon status (workspace, uptime, servers) |

#### Health Check

```
GET /api/v1/health
```

Response:
```json
{
  "success": true,
  "data": {
    "status": "ok"
  }
}
```

#### Status

```
GET /api/v1/status
```

Response:
```json
{
  "success": true,
  "data": {
    "workspace": "/home/user/projects/myapp",
    "uptime_secs": 3600,
    "servers": [
      {
        "language": "rust",
        "workspace": "/home/user/projects/myapp",
        "state": {
          "status": "running",
          "at": "2024-01-15T10:30:00Z"
        }
      }
    ]
  }
}
```

**Server State Format** (tagged enum with `status` field):

| Status     | Fields                           | Description                |
| ---------- | -------------------------------- | -------------------------- |
| `starting` | `at: datetime`                   | Server is initializing     |
| `running`  | `at: datetime`                   | Server is ready            |
| `stopped`  | `at: datetime`                   | Server has stopped         |
| `failed`   | `at: datetime`, `reason: string` | Server failed to start/run |

### 2. LSP Server Management

| Method | Endpoint                    | Description                     |
| ------ | --------------------------- | ------------------------------- |
| GET    | `/api/v1/lsp/servers`       | List running LSP servers        |
| POST   | `/api/v1/lsp/servers/start` | Start LSP server for a language |
| POST   | `/api/v1/lsp/servers/stop`  | Stop LSP server                 |

Note: LSP servers are scoped to the daemon's workspace. No workspace parameter needed.

#### List LSP Servers

```
GET /api/v1/lsp/servers
```

Response:
```json
{
  "success": true,
  "data": {
    "servers": [
      {
        "language": "rust",
        "workspace": "/home/user/projects/myapp",
        "state": {
          "status": "running",
          "at": "2024-01-15T10:30:00Z"
        },
        "progress": [
          {
            "title": "Indexing",
            "message": "Processing crates",
            "percentage": 50
          }
        ]
      }
    ]
  }
}
```

**Progress Info** (optional, present during LSP initialization):
- `title`: Progress title (optional)
- `message`: Progress message (optional)
- `percentage`: 0-100 progress value (optional)

#### Start LSP Server

```
POST /api/v1/lsp/servers/start
Content-Type: application/json

{
  "language": "rust"
}
```

Response:
```json
{
  "success": true,
  "data": {
    "language": "rust",
    "workspace": "/home/user/projects/myapp",
    "state": {
      "status": "running",
      "at": "2024-01-15T10:30:00Z"
    }
  }
}
```

#### Stop LSP Server

```
POST /api/v1/lsp/servers/stop
Content-Type: application/json

{
  "language": "rust",
  "force": false
}
```

| Field      | Type    | Required | Description                                   |
| ---------- | ------- | -------- | --------------------------------------------- |
| `language` | string  | yes      | Language to stop                              |
| `force`    | boolean | no       | Force stop if server is busy (default: false) |

Response:
```json
{
  "success": true,
  "data": {
    "language": "rust",
    "workspace": "/home/user/projects/myapp",
    "state": {
      "status": "stopped",
      "at": "2024-01-15T10:35:00Z"
    }
  }
}
```

### 3. LSP Operations

| Method | Endpoint                  | Description      |
| ------ | ------------------------- | ---------------- |
| POST   | `/api/v1/lsp/definition`  | Go to definition |
| POST   | `/api/v1/lsp/references`  | Find references  |
| POST   | `/api/v1/lsp/hover`       | Get hover info   |
| POST   | `/api/v1/lsp/completions` | Get completions  |
| POST   | `/api/v1/lsp/diagnostics` | Get diagnostics  |
| POST   | `/api/v1/lsp/rename`      | Rename symbol    |

Note: No `workspace_path` parameter needed - workspace is determined by the daemon instance.

#### LSP Request Format

All LSP operations require **absolute file paths**:

```json
{
  "language": "rust",
  "file_path": "/home/user/projects/myapp/src/main.rs",
  "position": {
    "line": 10,
    "character": 5
  }
}
```

**Important**:
- `file_path` must be an **absolute path** (LSP protocol requirement for `file://` URIs)
- `line` and `character` are **0-indexed**

#### Go to Definition

```
POST /api/v1/lsp/definition
Content-Type: application/json

{
  "language": "rust",
  "file_path": "/home/user/projects/myapp/src/main.rs",
  "position": { "line": 10, "character": 5 }
}
```

Response:
```json
{
  "success": true,
  "data": {
    "locations": [
      {
        "file_path": "/home/user/projects/myapp/src/lib.rs",
        "range": {
          "start": { "line": 25, "character": 0 },
          "end": { "line": 30, "character": 1 }
        }
      }
    ]
  }
}
```

Empty result when no definition found:
```json
{
  "success": true,
  "data": {
    "locations": []
  }
}
```

#### Find References

```
POST /api/v1/lsp/references
Content-Type: application/json

{
  "language": "rust",
  "file_path": "/home/user/projects/myapp/src/main.rs",
  "position": { "line": 10, "character": 5 }
}
```

Response:
```json
{
  "success": true,
  "data": {
    "locations": [
      {
        "file_path": "/home/user/projects/myapp/src/lib.rs",
        "range": {
          "start": { "line": 25, "character": 0 },
          "end": { "line": 30, "character": 1 }
        }
      }
    ]
  }
}
```

#### Get Hover

```
POST /api/v1/lsp/hover
Content-Type: application/json

{
  "language": "rust",
  "file_path": "/home/user/projects/myapp/src/main.rs",
  "position": { "line": 10, "character": 5 }
}
```

Response:
```json
{
  "success": true,
  "data": {
    "contents": "```rust\nfn main() -> ()\n```"
  }
}
```

No hover info available:
```json
{
  "success": true,
  "data": {
    "contents": null
  }
}
```

#### Get Completions

```
POST /api/v1/lsp/completions
Content-Type: application/json

{
  "language": "rust",
  "file_path": "/home/user/projects/myapp/src/main.rs",
  "position": { "line": 10, "character": 5 }
}
```

Response:
```json
{
  "success": true,
  "data": {
    "items": [
      {
        "label": "main",
        "detail": "fn main() -> ()",
        "documentation": "The main function"
      }
    ]
  }
}
```

#### Get Diagnostics

```
POST /api/v1/lsp/diagnostics
Content-Type: application/json

{
  "language": "rust",
  "file_path": "/home/user/projects/myapp/src/main.rs"
}
```

Response:
```json
{
  "success": true,
  "data": {
    "diagnostics": [
      {
        "range": {
          "start": { "line": 10, "character": 4 },
          "end": { "line": 10, "character": 5 }
        },
        "severity": "error",
        "message": "cannot find value `x` in this scope"
      }
    ]
  }
}
```

**Severity values**: `error`, `warning`, `information`, `hint`, `unknown`

#### Rename Symbol

```
POST /api/v1/lsp/rename
Content-Type: application/json

{
  "language": "rust",
  "file_path": "/home/user/projects/myapp/src/main.rs",
  "position": { "line": 10, "character": 5 },
  "new_name": "new_variable_name"
}
```

Response:
```json
{
  "success": true,
  "data": {
    "changes": [
      {
        "file_path": "/home/user/projects/myapp/src/main.rs",
        "edits": [
          {
            "range": {
              "start": { "line": 10, "character": 4 },
              "end": { "line": 10, "character": 8 }
            },
            "new_text": "new_variable_name"
          }
        ]
      }
    ]
  }
}
```

No changes (symbol not found or cannot be renamed):
```json
{
  "success": true,
  "data": {
    "changes": []
  }
}
```

### 4. Text Document Synchronization

| Method | Endpoint                | Description             |
| ------ | ----------------------- | ----------------------- |
| POST   | `/api/v1/lsp/didOpen`   | Notify document opened  |
| POST   | `/api/v1/lsp/didChange` | Notify document changed |
| POST   | `/api/v1/lsp/didClose`  | Notify document closed  |

These notifications keep the LSP server in sync with the current state of documents being edited.

#### Document Open

```
POST /api/v1/lsp/didOpen
Content-Type: application/json

{
  "language": "rust",
  "file_path": "/home/user/projects/myapp/src/main.rs",
  "language_id": "rust",
  "version": 1,
  "text": "fn main() {\n    println!(\"Hello\");\n}"
}
```

Response:
```json
{
  "success": true,
  "data": null
}
```

#### Document Change

```
POST /api/v1/lsp/didChange
Content-Type: application/json

{
  "language": "rust",
  "file_path": "/home/user/projects/myapp/src/main.rs",
  "version": 2,
  "content_changes": [
    {
      "range": {
        "start": { "line": 1, "character": 4 },
        "end": { "line": 1, "character": 9 }
      },
      "text": "Goodbye"
    }
  ]
}
```

**Content Change**:
- `range`: Optional. If omitted, the change is treated as a full document replacement
- `text`: The new text for the specified range

Response:
```json
{
  "success": true,
  "data": null
}
```

#### Document Close

```
POST /api/v1/lsp/didClose
Content-Type: application/json

{
  "language": "rust",
  "file_path": "/home/user/projects/myapp/src/main.rs"
}
```

Response:
```json
{
  "success": true,
  "data": null
}
```

---

## CLI Usage

### Start Daemon

```bash
# Start daemon for a workspace (auto-detects languages)
aifed-daemon --workspace /path/to/project

# With custom socket path
aifed-daemon --workspace /path/to/project --socket /tmp/custom.sock

# With custom idle timeout (default: 1800 = 30 minutes)
aifed-daemon --workspace /path/to/project --idle-timeout-secs 3600
```

### Test with curl

```bash
# Health check
curl --unix-socket ~/.cache/aifed/myapp-xxx.sock http://localhost/api/v1/health

# Status
curl --unix-socket ~/.cache/aifed/myapp-xxx.sock http://localhost/api/v1/status

# Start LSP server
curl --unix-socket ~/.cache/aifed/myapp-xxx.sock \
  -X POST \
  -H "Content-Type: application/json" \
  -d '{"language": "rust"}' \
  http://localhost/api/v1/lsp/servers/start
```

---

## Language Detection

The daemon automatically detects languages by checking for project files in the workspace root:

| File(s)                                          | Language   |
| ------------------------------------------------ | ---------- |
| `Cargo.toml`                                     | rust       |
| `package.json`                                   | javascript |
| `package.json` + `tsconfig.json`                 | typescript |
| `go.mod`                                         | go         |
| `pyproject.toml`, `setup.py`, `requirements.txt` | python     |

---

## Error Codes

| Code               | Description                           |
| ------------------ | ------------------------------------- |
| `INVALID_PATH`     | Invalid file path (must be absolute)  |
| `LSP_START_FAILED` | Failed to start LSP server            |
| `LSP_STOP_FAILED`  | Failed to stop LSP server             |
| `LSP_SERVER_BUSY`  | LSP server is busy (try with `force`) |
| `LSP_ERROR`        | General LSP operation error           |

---

## Summary

| Module    | Endpoints |
| --------- | --------- |
| Daemon    | 2         |
| LSP       | 12        |
| **Total** | **14**    |
