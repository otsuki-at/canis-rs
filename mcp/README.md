# canis MCP Server
canis によって収集した証跡から，論文についてその論文中の図の情報をまとめた説明作成のためのMCPサーバです．
利用例として，論文の元データなどについて，いつから存在したかといった情報を証明するための説明をLLMに作成させます．

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
      "/path/to/canis_mcp",
      "run",
      "main.py",
      "/path/to/canis/database"
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
        "/path/to/canis_mcp",
        "run",
        "main.py",
        "/path/to/canis/database"
      ],
    }
  }
}
```

### 各パスの説明
- `path/to/canis_mcp` : `canis_mcp` ディレクトリのパス
- `/path/to/canis/database` : canisが証跡のログを保存しているデータベースのパス

## Usage
論文内の図や表についての説明を作成する際の手順の例を以下に説明します．
手順2〜5において LLM に送信するプロンプトの具体例は，[examples/generate_explain_prompt.md](./examples/generate_explain_prompt.md) にまとめています。

1. 論文の図や表の位置にコメントとして，作成用のスクリプトやプログラムのファイル名を記載しておきます．  
LLM に原稿の内容を確認させる際，図や表をどのようなスクリプトやプログラムで作成したかを理解できるようにするために記載します．
以下に図をシェルスクリプトを利用して作成した場合の例を示します．
```tex
  \myinsertfigure{temp_elec}
  % plot_temp_electricity.sh で作成
```
2. LLMに原稿を送信し，図表作成用ファイルを探すように指示します．  
LLM に原稿内の図表作成用ファイルを見つけさせるために原稿を送信し，探索するように指示します．
手順1 で図表作成用ファイルをコメントとして残しているので， LLM に図や表の挿入位置を見つけ，作成用のファイルを認識させます．

3. LLM に図表作成用ファイルの中から元データを推測するように指示します．  
LLM に各図の元データを推測させるために図表作成用ファイルの内容を確認するように指示します．
手順2 で見つけた図表作成用ファイルには，作成用のプログラムやそこで利用している元データのファイルなどが記載されていると考えられるため，LLM にそこから元データのファイルを推測させます．

4. LLM に各図の元データに関する情報を集めるように指示します．  
LLM に各図の元データについての情報を作成・収集させます．
手順3 で推測した元データのファイルについて，LLM に以下のような情報を作成・収集するように指示します．
   - ハッシュ値の計算
   - 存在した日時の取得
   - 存在した日付の証跡の一覧ファイルの作成
   - 存在した日についての日次ハッシュ値の計算

5. LLM に各図の元データに関する説明を作成するように指示します．  
元データの情報について説明するファイルを作成するために， LLM に集めた情報を元に各図の元データに関する説明を作成するように指示します．
