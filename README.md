# canis-rs
canis は，計算機上でのユーザのファイル操作を監視し，ファイル操作を契機として決められた処理を行うためのシステムです．利用例として，ハッシュを作成・保存することで研究データの存在を証明する証拠とします．

Windows， MacOS， Linux で動作するシステムで，それぞれのOSでは以下のような監視システムを利用しています．
- Windows: ReadDirectoryChangesExW
- MacOS: FSEvents
- Linux: inotify
Filesystem in Userspace (FUSE) を利用することで，より高度な監視ができます．

また，canis は以下の機能を提供しています．
- ユーザのファイル操作を監視して証跡を保存する
- 指定したファイルの証跡について検索する
- 指定したファイルについて履歴を表示する (TODO)
- 1日に1回その日に作成した証跡からハッシュを作成し，公開する
- 論文中の図の情報をまとめた説明を作成する

# Quick start
canis は，バックグラウンドで動作してユーザのファイル操作を監視し，以下の項目からなる証跡を作成します．
- 操作時刻
- ハッシュ値
- ファイルパス

以下にシステムを利用して監視し，証跡を保存する動作の例を示します．
以下で示すコマンドの詳細については，以降の Usage に記載しています．

1. まずは，ユーザの操作監視を開始するためにシステムを起動します．
'''
  $ canis start --watcher notify --targets /watching/path/1/ --logfile /path/to/logfile
'''
上記のコマンドは，システムを起動するためのコマンド `start` です．
それぞれの引数は以下の項目を設定しています．
- `watcher`: 監視に利用するシステム
- `targets`: 監視対象のパス
- `logfile`: 証跡を保存するログファイルのパス

2. システムを起動した状態で，監視パス配下でファイル操作を行います．
3. 操作したファイルについての証跡が保存されます．
保存される証跡の形式は以下のようになっています．
```
  2026-02-01T02:03:04.230445,ad56fcd90f67e70ba2f6d35779a856e70d32310cffdf61eda9885298bb83e595,/watching/path/1/test.txt
```


上記は簡単な操作例です．
これらに加えて複数のパス配下の監視やデーモンとしての起動も可能となっています．詳細については以下を確認してください

<!-- # Requirements
## All Platforms
- Python 3.x (optional, required only when using the MCP)

## Linux
- systemd
- [libfuse3.14.0](https://github.com/libfuse/libfuse) (optional, required only when using the FUSE backend)

## Windows
- [WinSW 3.x](https://github.com/winsw/winsw)
  - Windows はバックグラウンドで常に動かすプログラムを「サービス」という形で管理します．
  しかし， canis は元々サービス化される仕様になっていないため，WinSW を利用して Windows サービスとして登録・起動・停止・監視できるようにします．

## macOS
- launchd
- [MacFUSE](https://github.com/macfuse/macfuse) (optional, required only when using the FUSE backend) -->


# Installation
1. Releases から各 OS に対応したバイナリをダウンロードしてください

2. ダウンロードしたバイナリを任意のパスに移動して利用することができます．各 OS について移動先の例を以下に示します．
- Linux: ~/bin/
- Windows: C:\Users\<username>\bin\
- MacOS: ~/bin/

MacOSの場合は，実行するためには以下の操作が必要になります．
  - 実行権限の付与
  - ダウンロードしたファイルが安全な開発元によるものか検証し，マルウェアの実行を防止するセキュリティ機能を無効にする

これらの操作を行うためには，以下のコマンドを実行してください．
```
  $ chmod +x canis
  $ xattr -d com.apple.quarantine canis
```

# Configuration
## Collect Component, Search Component
システムが利用する監視システムや監視対象のパス，証跡の保存先などを指定するために設定ファイルを利用できます．
設定ファイルは任意の場所に配置できます．例として，各 OS について以下の場所をデフォルトとして設定しています．
- Linux: `/home/<user>/.config/canis/config.toml`
- Windows: `C:\Users\<user>\AppData\Roaming\canis\config\config.toml`
- MacOS: `/Users/<user>/Library/Application/canis/config.toml`

設定ファイルには以下の情報を記載できます．
- watcher: 監視に利用するシステムを指定します． notify または fuse を指定できます． ただし，fuse を指定したい場合は以降に示す FUSE による監視を利用するための手順を実行してください．
- targets: 監視したいディレクトリのパスを指定します．notify を利用する場合は複数指定可能です．FUSE を利用する場合は 1つ目の要素を監視対象パスとして選択します．
- logfile: 証跡を保存するログファイルの配置場所を指定します．引数および設定ファイルで指定されていない場合は各 OS ごとに以下の場所に作成されます．
  - Linux: `/home/<user>/.local/share/canis/canis.log`
  - Windows: `C:\Users\<user>\AppData\Roaming\canis\data\canis.log`
  - MacOS: `/Users/<user>/Library/Application/canis/canis.log`
- dailyhash_repository: 日次ハッシュを公開するGitリポジトリをクローンしたローカルリポジトリのパスを指定します．

以下に設定ファイルの例を示します．
  ```
  [basic_settings]
  # Select the watcher implementation: notify or fuse
  watcher = "notify"

  # Specify the paths of files or directories to monitor
  # When using FUSE, only the first path in this list will be monitored
  targets = ["/watching/path/1/","/watching/path/2/"]

  # Specify the path to the log file where digest will be stored
  # Do not place the log file under any monitored path to avoid infinite loops
  logfile = "/path/to/logfile"

  # Specify the path to a local clone of the Git repository
  # This repository is used to publish daily hash values
  hashdir = "/path/to/local/gitrepository/"
  ```

## 証跡公開先リポジトリの準備
1日の証跡全体から作成したハッシュ (日次ハッシュ) を外部に公開する機能を提供しています．
この公開先として利用するための Git リポジトリを作成してください．
1. 日次ハッシュ公開先として利用する Git リポジトリを作成します．
2. 作成した Git リポジトリをローカルに Clone してください．ここでは，daily-hashというリポジトリを例に示しています．
  ```
  $ git clone https://github.com/User/daily-hash.git
  ```

## Model Context Protocol
canis が保存した証跡などの情報から，AI を利用して論文中の図の情報をまとめた説明を作成できます．
この機能を利用するための Model Context Protocol (MCP) の設定を以下に記載します．
- Configuration for Claude.app
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

- Configuration for VS code
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

- `path/of/canis_mcp` : `canis_mcp` ディレクトリのパス
- `path/of/canis/log` : canisが証跡のログを保存しているファイルのパス


# Usage
## Commands
- canis -h
```
Timestamping System for Research Data Management

Usage: canis <COMMAND>

Commands:
  init     Generate template files for configuration
  start    Start canis system
  stop     Stop canis system
  info     Display digest about file
  publish  Publish daily hash
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

- canis init -h
```
Generate template files for configuration

Usage: canis init [OPTIONS]

Options:
  -c, --config <CONFIG>    Path to configuration file
      --watcher <WATCHER>  File system watcher backend to use
      --targets <TARGETS>  Paths to watch for operation
      --logfile <LOGFILE>  Path to log file for digest
      --hashdir <HASHDIR>  Local git repository path for hash storage and publication
  -s, --start <START>      Generate service file for monitoring (canis start)
  -b, --binary <BINARY>    Path to canis binary file
  -p, --publish            Generate only service file for daily publishing (canis publish)
  -h, --help               Print help
```

- canis start -h
```
Start canis system

Usage: canis start [OPTIONS]

Options:
  -c, --config <CONFIG>    Specify config file
      --watcher <WATCHER>  File system watcher backend to use
      --targets <TARGETS>  Paths to watch for operation
      --logfile <LOGFILE>  Path to log file for digest
  -d, --daemon             Start background
  -h, --help               Print help
```

- canis stop -h
```
Stop canis system

Usage: canis stop

Options:
  -h, --help  Print help
```

- canis info -h (TODO)
```
Display digest about file

Usage: canis info [OPTIONS] <FILEPATH>

Arguments:
  <FILEPATH>  Path to the file to search

Options:
  -l, --log   Generate log about file
  -h, --help  Print help
```

- canis publish -h (TODO)
```
Publish daily hash

Usage: canis publish

Options:
  -h, --help  Print help
```

## examples
### init コマンド
```
canis init
```
canis の設定を記載できる設定ファイルや自動起動のための設定ファイルを生成します．

ファイルの配置場所や設定項目の内容は，引数で指定できます．
また，引数で指定しない場合は `init` コマンド実行後に対話的に設定できます．

以下にそれぞれのファイルを生成した場合の例について示します．
1. システムの起動に必要な設定を記載する設定ファイル

それぞれの OS について，デフォルトの配置場所は以下に設定しています．
- Linux: `/home/<user>/.config/canis/config.toml`
- Windows: `C:\Users\<user>\AppData\Roaming\canis\config\config.toml`
- MacOS: `/Users/<user>/Library/Application/canis/config.toml`

ファイルの内容の例を以下に示します．
```
  [basic_settings]
  # Select the watcher implementation: notify or fuse
  watcher = "notify"

  # Specify the paths of files or directories to monitor
  # When using FUSE, only the first path in this list will be monitored
  targets = ["/watching/path/1/","/watching/path/2/"]

  # Specify the path to the log file where digest will be stored
  # Do not place the log file under any monitored path to avoid infinite loops
  logfile = "/path/to/logfile"

  # Specify the path to a local clone of the Git repository
  # This repository is used to publish daily hash values
  hashdir = "/path/to/local/gitrepository/"
```
2. canis start を自動起動するための設定ファイル
  - Linux

  デフォルトの配置場所は
  `/home/<user>/.config/systemd/user/canis.service`
  に設定しています．

  ファイルの内容の例を以下に示します．
  ```
  [Unit]
  Description=canis

  [Service]
  Type=simple
  ExecStart = /home/user/bin/canis start --config /home/user/.config/canis/config.toml
  Restart=always

  [Install]
  WantedBy=default.target
  ```
  - Windows

  デフォルトの配置場所は
  `C:\Users\<user>\AppData\Roaming\canis\config\canis.xml`
  に設定しています．

  ファイルの内容の例を以下に示します．
  ```
  <service>
    <id>canis</id>
    <name>canis</name>
    <description>canis</description>
    <executable>C:\Users\user\bin\canis.exe</executable>
    <arguments>start --config C:\Users\user\AppData\Roaming\canis\config\config.toml</arguments>
    <serviceaccount>
      <username>{domain}\{username}</username>
      <password>passwd</password>
      <allowservicelogon>true</allowservicelogon>
    </serviceaccount>
  </service>
  ```
  - MacOS

  デフォルトの配置場所は
  `~/Library/LaunchAgents/com.canis.start.plist`
  に設定しています．

  ファイルの内容の例を以下に示します．
  ```
  <?xml version="1.0" encoding="UTF-8"?>
  <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
    "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
  <plist version="1.0">
  <dict>
      <key>Label</key>
      <string>com.{username}.canis</string>

      <key>ProgramArguments</key>
      <array>
          <string>/Users/user/bin/canis</string>
          <string>start</string>
          <string>--config</string>
          <string>/Users/user/Library/Application/canis/config.toml</string>
      </array>

      <key>KeepAlive</key>
      <true/>

      <key>RunAtLoad</key>
      <true/>

      <key>StandardOutPath</key>
      <string>/User/user/Library/Logs/canis.log</string>

      <key>StandardErrorPath</key>
      <string>/User/user/Library/Logs/canis.err</string>
  </dict>
  </plist>
  ```
3. canis publish を定期実行するファイル(TODO)

### info コマンド(TODO)
```
canis info FILEPATH
```
保存した証跡から `FILEPATH` について検索し，以下のような情報を出力します．
```
作成時刻: 2026-02-01T02:03:04.230445
ファイル: /home/user/test.txt
ハッシュ: ad56fcd90f67e70ba2f6d35779a856e70d32310cffdf61eda9885298bb83e595
```
ここから，検索した `FILEPATH` のハッシュ値となるファイルがいつから存在していたかを示すために利用できます．


# Advanced Setting
## FUSE を利用した監視機能の利用
ファイル操作監視システムとして FUSE を利用することで，より詳細な操作を取得できるようになります．
FUSE による監視機能を利用できるようにするために手元でビルドする必要があります．

### Requires
- Linux
  - [libfuse3.14.0](https://github.com/libfuse/libfuse)
- MacOS
  - [MacFUSE](https://github.com/macfuse/macfuse)

### ビルド手順
ビルドの手順を以下に示します．

1. 本リポジトリを clone します
```
 $ git clone git@github.com:nomlab/canis-rs.git
```

2. FUSE に関する機能を含めてビルドします

以下のコマンドで FUSE に関わる機能を含めてビルドを実行できます．
```
  $ cargo build --release --features fuse
```

## システムの自動起動設定
## 自動起動設定
canis を計算機起動時に自動で起動するように設定することで，ユーザが毎回起動せずともファイルアクセス監視が開始できます．

### Requires
- Linux
  - systemd

- Windows
  - [WinSW 3.x](https://github.com/winsw/winsw)
    - Windows はバックグラウンドで常に動かすプログラムを「サービス」という形で管理します．
      しかし， canis は元々サービス化される仕様になっていないため，WinSW を利用して Windows サービスとして登録・起動・停止・監視できるようにします．

- macOS
  - launchd

### 設定手順
自動起動を実現するための各 OS ごとの手順について以下に示します．

### Linux
1. 自動起動設定用のファイルを作成します

以下のコマンドでユニットファイルが作成できます．
```
canis init --start /path/to/unitfile --binary /path/to/canis/binary
```
それぞれの引数は以下の項目を設定しています．
- `start`: ユニットファイルの配置場所
- `binary`: canis のバイナリの配置場所

2. systemd の設定を再読み込みしてください

1 で作成した自動起動用の設定ファイルを読み込む必要があるため，以下のコマンドを実行します．
```
systemctl --user daemon-reload
```
3. canis start が自動起動するよう設定します

以下のコマンドでシステム起動時に canis が自動起動するようになります．
```
systemctl --user enable canis-start.service
systemctl --user start canis-start.service
```

### Windows
1. WinSW 3.x をインストールしてください

以下のサイトからインストールできます．

https://github.com/winsw/winsw

以下のコマンドでヘルプが表示されればインストールできています．
```
  $ WinSW-x64.exe -h
```

2. 自動起動設定用のファイルを作成します．

以下のコマンドによって WinSW 用の設定ファイルが作成できます．
```
$ canis.exe init --start /path/to/unitfile --binary /path/to/canis/binary
```
それぞれの引数は以下の項目を設定しています．
- `start`: WinSW 用の設定ファイルの配置場所
- `binary`: canis のバイナリの配置場所

3. 作成された xml ファイルの password 欄にパスワードを入力します

ユーザのサービスとして起動するためには，ユーザ名およびパスワードを記載する必要があります．
そのため，作成された xml ファイルにパスワードを記入してください．以下にパスワードを記載する例を示します．
```
    <password>passwd</password>
```

3. canis をサービスとして登録します

作成した設定ファイルを利用して， canis をユーザのサービスとして登録します．
以下に登録するためのコマンド例を示します．ここでは，設定ファイルがデフォルトの配置場所にある例を示しています．
```
$ WinSW-x64.exe install C:\Users\<user>\AppData\Roaming\canis\config\canis.xml
```

4. canis のサービスを実行します

登録したサービスを実行し， canis を起動します．
以下にサービスを実行するためのコマンド例を示します．3と同様に設定ファイルがデフォルトの配置場所にある例を示しています．
```
$ WinSW-x64.exe start C:\Users\<user>\AppData\Roaming\canis\config\canis.xml
```

### MacOS
1. 自動起動設定用のファイルを作成します

以下のコマンドで自動起動用の設定ファイルが作成できます．
```
canis init --start /path/to/unitfile --binary /path/to/canis/binary --daemon_out /path/to/stdout/logfile--daemon_err /path/to/stderr/logfile
```
それぞれの引数は以下の項目を設定しています．
- `start`: ユニットファイルの配置場所
- `binary`: canis のバイナリの配置場所
- `daemon_out`: 標準出力用のログファイルの配置場所
- `daemon_err`: 標準エラー出力用のログファイルの配置場所

2. canis start が自動起動するよう設定します

以下のコマンドで自動起動用の設定を読み込み，自動起動を有効化します．
```
launchctl load ~/Library/LaunchAgents/com.canis.start.plist
```

# For Developers
## システム構成
![#Overview](./doc/canis_rs_all.svg)

1. 本システムの証跡を収集する機能は，以下の要素から構成されている．
   - File Access Monitor: ユーザのファイル操作を監視し，証跡を作成するべき操作を取得した場合はレベル変換部にファイル操作情報を通知する
   - Level Converter: ファイルアクセス監視部と証跡作成部間のアダプタ．取得したファイル操作情報を証跡作成部が対応している操作の情報に変換し，通知する
   - Digest Generator: 取得したファイル操作情報に基づいて証跡を作成し，証跡保存データベースに保存する
   - Digest Database: システムが作成した証跡を保存するデータベース
   - Hash Generator: ユーザが指定したファイルのハッシュ値を作成する
   - Hash Searcher: ユーザが指定したファイルのハッシュ値が保存されているか検索し，結果を表示する
   - Daily Hash Generator: 1日に1回その日の証跡一覧から日時ハッシュを取得し，外部に公開する
   - MCP Host: ユーザからの説明作成依頼を LLM に送信する．また，LLM から tool 利用に必要な情報を取得し，MCP クライアントから tool の実行結果を取得する．
   - MCP Client: MCP ホストから tool 利用に必要な情報を取得し，情報に基づいて必要な tool を呼び出す．呼び出した tool の動作結果を取得し，MCP ホストに送信する．
   - MCP Server: MCP クライアントから取得した tool 利用情報をもとに対応した tool を実行する．実行結果は MCP クライアントに送信する．
   - LLM: MCP ホストから説明作成依頼を取得し，説明を作成する．また，説明作成に必要な情報を取得するために tool に必要な情報を MCP ホストに送信する．

## インタフェース
File Access Monitor から Level Converter および， Level Converter から Digest Generator は以下のインタフェースに基づいたメッセージが通知される．
 - レベル1
   - create(filepath,time)
   - modify(filepath,time)
 - レベル2
   - move(filepath1, filepath2,time)
 - レベル3
   - open(filepath,pid,time)
   - write(filepath,content,time)
   - append(filepath, content,time)
