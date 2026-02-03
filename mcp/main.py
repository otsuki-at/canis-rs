from mcp.server import Server
from mcp.server.stdio import stdio_server
from mcp.types import Tool, TextContent, ImageContent, EmbeddedResource, Prompt, PromptArgument, GetPromptResult, PromptMessage
from mcp.shared.exceptions import McpError

import json
from typing import Sequence

import subprocess

import asyncio
import sys

class CanisServer:
    def __init__(self, hashlog: str):
        self.hashlog = hashlog

    def create_hash(self, filepath: str) -> str:
        """Create hash from specified file"""
        hash = subprocess.run(["sha256sum", filepath], capture_output=True, text=True).stdout
        return hash

    def search_target_from_log(self, target: str) -> str:
        """Search for specified file or hash value in hash log"""
        result = subprocess.run(["grep", target, self.hashlog], capture_output=True, text=True).stdout
        if not result:
            return "Target not found in hash log."
        return result

    def create_daily_log_file(self, day:str, path:str):
        """Create a hash from logs obtained on a specific date"""
        day = day +"T"
        log = subprocess.run(["grep", day, self.hashlog], capture_output=True, text=True).stdout
        with open(path, "w") as f:
            print(log, file=f, end = "")

        return f"Hash log saved to: {path}"


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
        print("Usage: python main.py <hashlog_path>")
        sys.exit(1)
    hashlog = sys.argv[1]
    asyncio.run(serve(hashlog))


if __name__ == "__main__":
    main()
