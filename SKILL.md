---
name: gizza-tools
description: Run gizza's local compute tools (calculator, clock, image/video, fetch, …) via the `gizza` CLI binary installed in the repo.
---

## Usage

All tools are invoked through the `gizza` binary.  Run from the `gizza-ai` repo root.

### Invoke a tool

```sh
# Positional: bare values fill required scalar fields left-to-right
gizza tool <name> <arg>

# Key=value pairs (mixed freely with positionals)
gizza tool <name> key=value ...

# Full JSON body
gizza tool <name> --json '{"key":"value"}'

# Print the complete JSON response envelope
gizza tool <name> <args> --json-out

# Write binary output to a file (image, video, …)
gizza tool <name> <args> --out result.png
```

### Discover tools

```sh
gizza list                 # table of short-name + description
gizza list --json-out       # JSON array
gizza describe <name>       # full schema for one tool
gizza describe <name> --json-out
```

### Generate / check this file

```sh
gizza gen-skill             # (re)write SKILL.md in the current directory
gizza gen-skill --check     # exit 0 if up-to-date, exit 1 if stale
```

### Exit codes

| Code | Meaning |
|------|---------|
| 0    | Success |
| 1    | Tool error (invalid input, compute failure) |
| 2    | Usage error (unknown tool, missing required arg) |
| 3    | Unsupported in CLI (e.g. `imagine` requires a browser GPU; use gizza.ai) |

### Example

```sh
$ gizza tool calculator "2*2"
4
```

## Tools

### calculator

Evaluate an arithmetic expression (e.g. '2+2*3'). Returns the numeric result.

```json
{
  "additionalProperties": false,
  "properties": {
    "expr": {
      "description": "Arithmetic expression to evaluate (e.g. '2+2*3', 'sqrt(16)', '3.14 * 2^2').",
      "type": "string"
    }
  },
  "required": [
    "expr"
  ],
  "type": "object"
}
```

### clock

Get the current UTC time. Returns ISO 8601 timestamp.

```json
{
  "additionalProperties": false,
  "properties": {},
  "type": "object"
}
```

### ffmpeg

Run ffprobe on a media URL and return format/stream metadata.

```json
{
  "additionalProperties": false,
  "properties": {
    "url": {
      "description": "HTTP/HTTPS URL of the media file to inspect.",
      "type": "string"
    }
  },
  "required": [
    "url"
  ],
  "type": "object"
}
```

### image-convert

Convert an image to a different format (jpeg, png, webp). Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call).

```json
{
  "oneOf": [
    {
      "required": [
        "url"
      ]
    },
    {
      "required": [
        "ref"
      ]
    }
  ],
  "properties": {
    "format": {
      "description": "Target image format.",
      "enum": [
        "jpeg",
        "png",
        "webp"
      ],
      "type": "string"
    },
    "quality": {
      "description": "Output quality 1-100 (default 85, ignored for png).",
      "maximum": 100,
      "minimum": 1,
      "type": "integer"
    },
    "ref": {
      "description": "Reference id from a prior image tool call (e.g. \"call_42\"). Use either url or ref.",
      "type": "string"
    },
    "url": {
      "description": "Image URL (HTTP/HTTPS).",
      "type": "string"
    }
  },
  "required": [
    "format"
  ],
  "type": "object"
}
```

### image-crop

Crop a rectangular region from an image. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call).

```json
{
  "oneOf": [
    {
      "required": [
        "url"
      ]
    },
    {
      "required": [
        "ref"
      ]
    }
  ],
  "properties": {
    "height": {
      "description": "Height of the crop rectangle in pixels.",
      "minimum": 1,
      "type": "integer"
    },
    "ref": {
      "description": "Reference id from a prior image tool call (e.g. \"call_42\"). Use either url or ref.",
      "type": "string"
    },
    "url": {
      "description": "Image URL (HTTP/HTTPS).",
      "type": "string"
    },
    "width": {
      "description": "Width of the crop rectangle in pixels.",
      "minimum": 1,
      "type": "integer"
    },
    "x": {
      "description": "Left offset of the crop rectangle in pixels.",
      "minimum": 0,
      "type": "integer"
    },
    "y": {
      "description": "Top offset of the crop rectangle in pixels.",
      "minimum": 0,
      "type": "integer"
    }
  },
  "required": [
    "x",
    "y",
    "width",
    "height"
  ],
  "type": "object"
}
```

### image-fetch

Fetch an image from a URL and render it inline.

```json
{
  "additionalProperties": false,
  "properties": {
    "url": {
      "description": "HTTP/HTTPS URL of the image to fetch.",
      "type": "string"
    }
  },
  "required": [
    "url"
  ],
  "type": "object"
}
```

### image-grayscale

Convert an image to grayscale. Provide url (HTTP/HTTPS) or ref from a prior image tool call.

```json
{
  "oneOf": [
    {
      "required": [
        "url"
      ]
    },
    {
      "required": [
        "ref"
      ]
    }
  ],
  "properties": {
    "ref": {
      "description": "Reference id from a prior image tool call (e.g. \"call_42\"). Use either url or ref.",
      "type": "string"
    },
    "url": {
      "description": "Image URL (HTTP/HTTPS).",
      "type": "string"
    }
  },
  "type": "object"
}
```

### image-resize

Resize an image. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call).

```json
{
  "oneOf": [
    {
      "required": [
        "url"
      ]
    },
    {
      "required": [
        "ref"
      ]
    }
  ],
  "properties": {
    "fit": {
      "description": "Resize mode (default: contain).",
      "enum": [
        "contain",
        "cover",
        "stretch"
      ],
      "type": "string"
    },
    "height": {
      "description": "Target height in pixels.",
      "minimum": 1,
      "type": "integer"
    },
    "ref": {
      "description": "Reference id from a prior image tool call (e.g. \"call_42\"). Use either url or ref.",
      "type": "string"
    },
    "url": {
      "description": "Image URL (HTTP/HTTPS).",
      "type": "string"
    },
    "width": {
      "description": "Target width in pixels.",
      "minimum": 1,
      "type": "integer"
    }
  },
  "type": "object"
}
```

### imagine

Generate an image from a text prompt. Renders inline in the chat. Requires WebGPU in the browser (uses shader-f16 when available, falls back to fp32 otherwise). Output is a PNG.

```json
{
  "additionalProperties": false,
  "properties": {
    "prompt": {
      "description": "Description of the image to generate.",
      "type": "string"
    }
  },
  "required": [
    "prompt"
  ],
  "type": "object"
}
```

### video-frame-extract

Extract a single frame from a video at the given timestamp (seconds), output as PNG. The PNG is naturally chainable into image-resize, image-crop, or image-convert via ref. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call).

```json
{
  "oneOf": [
    {
      "required": [
        "url"
      ]
    },
    {
      "required": [
        "ref"
      ]
    }
  ],
  "properties": {
    "ref": {
      "type": "string"
    },
    "timestamp": {
      "description": "Timestamp in seconds.",
      "minimum": 0,
      "type": "number"
    },
    "url": {
      "type": "string"
    }
  },
  "required": [
    "timestamp"
  ],
  "type": "object"
}
```

### video-transcode

Transcode a video to a different format (mp4 or webm). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Quality 1-100 maps to ffmpeg CRF (default 75).

```json
{
  "oneOf": [
    {
      "required": [
        "url"
      ]
    },
    {
      "required": [
        "ref"
      ]
    }
  ],
  "properties": {
    "format": {
      "description": "Output container format.",
      "enum": [
        "mp4",
        "webm"
      ],
      "type": "string"
    },
    "quality": {
      "description": "Quality 1-100 (default 75). Lower = smaller file, lower quality.",
      "maximum": 100,
      "minimum": 1,
      "type": "integer"
    },
    "ref": {
      "description": "Reference id from a prior tool call (e.g. \"call_42\"). Use either url or ref.",
      "type": "string"
    },
    "url": {
      "description": "Video URL (HTTP/HTTPS).",
      "type": "string"
    }
  },
  "required": [
    "format"
  ],
  "type": "object"
}
```

### video-trim

Trim a video to a [start, start+duration] window using stream-copy (no re-encode). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Output is mp4. Stream-copy preserves the source codecs and is fast — but requires the source streams be mp4-compatible (h264/aac); otherwise ffmpeg will fail with a clear error.

```json
{
  "oneOf": [
    {
      "required": [
        "url"
      ]
    },
    {
      "required": [
        "ref"
      ]
    }
  ],
  "properties": {
    "duration": {
      "description": "Duration in seconds.",
      "exclusiveMinimum": 0,
      "type": "number"
    },
    "ref": {
      "type": "string"
    },
    "start": {
      "description": "Start time in seconds.",
      "minimum": 0,
      "type": "number"
    },
    "url": {
      "type": "string"
    }
  },
  "required": [
    "start",
    "duration"
  ],
  "type": "object"
}
```

### web-fetch

Fetch a URL and return its body as text. Optionally limit the response size.

```json
{
  "additionalProperties": false,
  "properties": {
    "max_bytes": {
      "description": "Maximum number of bytes to return (default: 1048576). Response is truncated if larger.",
      "minimum": 1,
      "type": "integer"
    },
    "url": {
      "description": "HTTP/HTTPS URL to fetch.",
      "type": "string"
    }
  },
  "required": [
    "url"
  ],
  "type": "object"
}
```

### word-count

Count the words, characters, and lines in a block of text.

```json
{
  "additionalProperties": false,
  "properties": {
    "text": {
      "description": "The text to analyze.",
      "type": "string"
    }
  },
  "required": [
    "text"
  ],
  "type": "object"
}
```

