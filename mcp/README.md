# Canis MCP Server
Canis によって収集した証跡から，論文についてその論文中の図の情報をまとめた説明作成のためのMCPサーバである．

### 提供する tool 一覧
- `create_hash` - 指定したファイルのハッシュ値を計算する
  - Required arguments
    - `filepath` (string): ハッシュ値を計算したいファイルのパス
- `search_target_from_log` - 指定した文字列について，証跡を保存したログから検索する
  - Required arguments
    - `target` (string): 検索したい文字列
- `create_daily_log_file` - 指定した日付の証跡のみを証跡を保存したログから取り出し，ファイルに出力する
  - Required arguments
    - `day` (string): 証跡を取り出したい日付
    - `path` (string): 取り出した証跡の一覧を出力したいファイルのパス

## Configuration
### Configuration for Claude.app
```json
{
  "mcpServers": {
    "canis": {
      "command": "uv",
      "args": [
      "--directory",
      "/path/of/canis_mcp",
      "run",
      "main.py",
      "/path/of/canis/log"
      ]
    }
  }
}
```

### Configuration for VS code
```json
{
  "servers": {
    "canis": {
      "command": "uv",
      "args": [
        "--directory",
        "/path/of/canis_mcp",
        "run",
        "main.py",
        "/path/of/canis/log"
      ],
    }
  }
}
```

### 各パスの説明
- `path/of/canis_mcp` : `canis_mcp` ディレクトリのパス
- `path/of/canis/log` : canisが証跡のログを保存しているファイルのパス
