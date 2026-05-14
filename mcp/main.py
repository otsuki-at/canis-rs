from mcp.server import Server
from mcp.server.stdio import stdio_server
from mcp.types import Tool, TextContent, ImageContent, EmbeddedResource, Prompt, PromptArgument, GetPromptResult, PromptMessage
from mcp.shared.exceptions import McpError

import json
from typing import Sequence

import sqlite3
import subprocess
from contextlib import contextmanager
from urllib.parse import urlparse
from pathlib import Path

import asyncio
import sys

class CanisServer:
    def __init__(self, db_path: str):
        self.db_path = db_path

    @contextmanager
    def _connect(self):
        con = sqlite3.connect(self.db_path)
        con.row_factory = sqlite3.Row
        try:
            yield con
        finally:
            con.close()

    def create_hash(self, filepath: str) -> str:
        """Create hash from specified file"""
        hash = subprocess.run(["sha256sum", filepath], capture_output=True, text=True).stdout
        return hash

    def search_target_from_log(self, target: str) -> str:
        """Search for specified file path or hash value in DB"""
        uri = self._normalize_to_uri(target)

        with self._connect() as con:
            rows = con.execute(
                """
                SELECT created_at, uri, hash
                FROM Digest
                WHERE (:uri IS NOT NULL AND uri = :uri) OR hash = :hash
                ORDER BY created_at DESC
                """,
                {"uri": uri, "hash": target},
            ).fetchall()

        if not rows:
            return "Target not found in hash log."

        return "\n".join(
            f"{row['created_at']}  {row['uri']}  {row['hash']}"
            for row in rows
        )

    def create_daily_log_file(self, day: str, path: str) -> str:
        """Fetch records for a specific date from DB and save to file"""
        with self._connect() as con:
            rows = con.execute(
                """
                SELECT created_at, uri, hash
                FROM Digest
                WHERE DATE(created_at) = DATE(:day)
                ORDER BY created_at
                """,
                {"day": day},
            ).fetchall()

        with open(path, "w") as f:
            for row in rows:
                print(f"{row['created_at']},{row['uri']},{row['hash']}", file=f)

        return f"Hash log saved to: {path}"

    @staticmethod
    def _normalize_to_uri(target: str) -> str:
        parsed = urlparse(target)
        if parsed.scheme:
            return target
        p = Path(target)
        if p.is_absolute():
            return p.as_uri()
        return None

async def serve(hashlog: str) -> None:
    server = Server("canis-mcp")
    canis_server = CanisServer(hashlog)

    @server.list_tools()
    async def list_tools() -> list[Tool]:
        """List available canis tools."""
        return [
            Tool(
                name="create_hash",
                description="Create a hash from specified file",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "filepath": {
                            "type": "string",
                            "description": "Filepath specified by the user.",
                        }
                    },
                    "required": ["filepath"],
                }
            ),
            Tool(
                name="search_target_from_log",
                description="Search for specified file or hash value in hash log",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Filepath or hash value specified by the user.",
                        }
                    },
                    "required": ["target",],
                }
            ),
            Tool(
                name="create_daily_log_file",
                description="Create a hash from logs obtained on a specific date",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "day": {
                            "type": "string",
                            "description": "Date specified by the user. The format must be '2025-06-01'.",
                        },
                        "path":{
                            "type":"string",
                            "description": "The path to the file where hash logs for a specific date are stored. If not specified by the user, the file will be automatically named using the format daily_hash_<day>.log"
                        }
                    },
                    "required": ["day","path"],
                }
            )
        ]

    @server.call_tool()
    async def call_tool(
        name: str, arguments: dict
    ) -> Sequence[TextContent | ImageContent | EmbeddedResource]:
        """Handle tool calls for canis queries."""
        try:
            match name:
                case "create_hash":
                    filepath = arguments.get("filepath")
                    if not filepath:
                        raise ValueError("Missing required argument: filepath")

                    result = canis_server.create_hash(filepath)

                case "search_target_from_log":
                    target = arguments.get("target")
                    if not target:
                        raise ValueError("Missing required argument: target")

                    result = canis_server.search_target_from_log(target)

                case "create_daily_log_file":
                    day = arguments.get("day")
                    path = arguments.get("path")
                    if not day:
                        raise ValueError("Missing required argument: day")
                    if not path:
                        raise ValueError("Missing required argument: path")

                    result = canis_server.create_daily_log_file(day, path)

            return [
                TextContent(type="text", text=result.strip())
            ]

        except Exception as e:
            raise ValueError(f"Error processing canis-mcp query: {str(e)}")

    options = server.create_initialization_options()
    async with stdio_server() as (read_stream, write_stream):
        await server.run(read_stream, write_stream, options)

def main():
    if len(sys.argv) < 2:
        print("Usage: python main.py <db_path>")
        sys.exit(1)
    db_path = sys.argv[1]
    asyncio.run(serve(db_path))


if __name__ == "__main__":
    main()
