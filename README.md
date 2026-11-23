# `@willh/say`

跨平台的 `say` 命令列語音合成（Text-to-Speech）工具，以 Rust 撰寫並採用 Microsoft Edge TTS（`msedge-tts`），高度相容 macOS 內建 `say` 的操作體驗，零外部執行期依賴（無需安裝 `ffmpeg`），預設語音為台灣繁體中文女聲 `zh-TW-HsiaoChenNeural`。

官方網站（含語音試聽）：<https://say.gh.miniasp.com/>

## 功能特色

- **跨平台原生效能**：以 Rust 編譯原生二進位檔，支援 macOS、Linux、Windows（含 x64 與 ARM64 共 6 大平台），透過薄型 npm 套件自動下載並驗證 SHA-256 校驗碼。
- **高度相容 macOS `say`**：支援位置參數、`-f/--input-file`、`stdin`（管線全文與 TTY 互動逐行朗讀）、`-v/--voice`、`-r/--rate`、`-o/--output-file`、`--file-format`、`--data-format`、`-a/--audio-device`、`--progress`、`-i/--interactive[=markup]`。
- **Edge TTS 擴充能力**：支援語音清單查詢與 `--locale` 過濾、模糊語音比對、音調 `--pitch` 與音量 `--volume` 調整，以及長文字自動智慧分段與音訊合併。
- **純 Rust 音訊播放與存檔**：內建 `rodio` + `cpal` 直接播放並選擇輸出裝置，或輸出為 `.mp3`、`.wav`、`.ogg`、`.webm` 音訊檔案。

## 安裝與快速開始

### 全域安裝（推薦）

```bash
npm install -g @willh/say
```

安裝完成後即可直接使用 `say` 指令：

```bash
say "你好世界"
say "Hello from cross-platform say!"
```

### 使用 `npx` 免安裝執行

```bash
npx @willh/say "你好，這是跨平台 say 工具！"
```

> 安裝時 `postinstall` 會從官方網站（<https://say.gh.miniasp.com/releases/>，內容與 GitHub Releases 相同）下載對應平台的原生二進位壓縮檔，並強制執行 SHA-256 校驗碼驗證。

### 一行指令安裝（不需要 Node.js）

macOS、Linux：

```bash
curl -fsSL https://say.gh.miniasp.com/install.sh | sh
```

Windows（PowerShell）：

```powershell
irm https://say.gh.miniasp.com/install.ps1 | iex
```

安裝腳本會從官方網站下載對應平台的執行檔、核對 SHA-256，再放進 `~/.local/bin`（Windows 是 `%USERPROFILE%\.local\bin`）。Windows 會自動把這個資料夾加進使用者的 `PATH`；macOS 與 Linux 如果還沒把它加進 `PATH`，腳本會印出要加的那一行指令。

可以用環境變數調整安裝方式：

| 環境變數 | 說明 |
| --- | --- |
| `SAY_INSTALL_DIR` | 安裝目錄，預設為 `~/.local/bin`。 |
| `SAY_VERSION` | 要安裝的版本，例如 `v0.1.0`；預設為最新版。 |
| `SAY_NO_MODIFY_PATH=1` | 僅 Windows：不要修改使用者的 `PATH`。 |

```bash
# 安裝指定版本到自訂目錄
curl -fsSL https://say.gh.miniasp.com/install.sh | SAY_INSTALL_DIR="$HOME/bin" sh -s -- --version v0.1.0
```

```powershell
$env:SAY_VERSION = 'v0.1.0'; irm https://say.gh.miniasp.com/install.ps1 | iex
```

macOS 內建了 `/usr/bin/say`，誰先被執行取決於 `PATH` 的順序；如果內建版本排在前面，安裝腳本會提醒你。要解除安裝，刪除 `~/.local/bin/say`（Windows 是 `~\.local\bin\say.exe`）即可。

## 參數說明

| 參數 | 環境變數 / 預設值 | 說明 |
| --- | --- | --- |
| `[MESSAGE]...` | — | 要朗讀的文字內容（多個參數以空白連接）。未提供且未指定 `-f` 時改由 `stdin` 讀取。 |
| `-v, --voice <VOICE>` | `SAY_VOICE`<br>預設：`zh-TW-HsiaoChenNeural` | 指定語音名稱。支援完整 `ShortName` 或不分大小寫模糊比對（如 `YunJhe`、`HsiaoChen`）；傳入 `?` 列出可用語音。 |
| `--list-voices` | — | 列出所有可用語音（可搭配 `--locale` 篩選）。 |
| `--locale <LOCALE>` | — | 搭配 `-v '?'` 或 `--list-voices`，依語系前綴過濾（例如 `zh-TW`、`en-US`、`ja`）。 |
| `-r, --rate <RATE>` | `+0%` | 語速調整，採**智慧雙模式**：<br>• 帶 `%` 或 `+/-` 符號（如 `+20%`、`-15%`）：視為 Edge TTS 百分比（範圍 `-100..=200`）。<br>• 純無號正數（如 `200`、`120`）：視為 macOS WPM，以 `175 WPM = 0%` 線性換算。 |
| `--pitch <PITCH>` | `+0Hz` | 調整音調（例如 `+10Hz`、`-5Hz`）。 |
| `--volume <VOLUME>` | `+0%` | 調整音量（例如 `+20%`、`-30%`）。 |
| `-o, --output-file <FILE>` | — | 將合成結果輸出為音訊檔，依副檔名自動選擇原生格式（預設 `.mp3`，支援 `.wav`、`.ogg`、`.webm`）。 |
| `--file-format <FORMAT>` | 依 `-o` 副檔名推斷 | 指定輸出格式（`mp3`、`wav`、`ogg`、`webm`），傳入 `?` 列出支援格式。 |
| `--data-format <FORMAT>` | — | 指定 Edge TTS 原生 `audio_format` 字串，傳入 `?` 列出支援的原生資料格式。 |
| `-f, --input-file <FILE>` | — | 從指定檔案讀取文字；傳入 `-` 表示從 `stdin` 讀取。 |
| `-a, --audio-device <DEVICE>` | 系統預設裝置 | 指定音訊輸出裝置的編號 ID 或名稱前綴（不分大小寫）；傳入 `?` 列出所有可用輸出裝置。 |
| `--progress` | `false` | 合成與播放期間於 `stderr` 顯示即時進度條。 |
| `-i, --interactive[=MARKUP]` | — | 播放時於終端機逐字高亮目前朗讀的文字。可選填樣式如 `bold`、`smul`、`smso`、顏色名稱（如 `green`、`cyan`）或 `fg/bg`（如 `yellow/blue`）。 |
| `-h, --help` | — | 顯示說明訊息。 |
| `-V, --version` | — | 顯示版本號。 |

## 使用範例

```bash
# 使用預設語音 (zh-TW-HsiaoChenNeural) 朗讀中文
say "早安，今天天氣真好！"

# 查詢所有台灣中文語音
say -v "?" --locale zh-TW
say --list-voices --locale en-US

# 使用模糊名稱切換語音（例如台灣男聲 zh-TW-YunJheNeural）
say -v YunJhe "歡迎使用跨平台 say 命令列工具"

# 透過環境變數設定預設語音
SAY_VOICE=en-US-JennyNeural say "Using a default voice from environment variable."

# 語速雙模式：Edge TTS 百分比 vs. macOS WPM
say -r +25% "加快百分之二十五語速"
say -r -15% "放慢語速朗讀"
say -r 210 "Speaking at 210 words per minute."

# 調整音調與音量
say --pitch +10Hz --volume +15% "提高音調並放大音量"

# 儲存為 MP3 / WAV / OGG / WEBM 音訊檔
say -o greeting.mp3 "這段語音會直接儲存為 MP3 檔案"
say -o announcement.wav -r +10% "這段語音會儲存為 WAV 檔案"

# 查詢支援的輸出格式與本機音訊裝置
say --file-format "?"
say --data-format "?"
say -a "?"

# 指定特定音訊輸出裝置播放
say -a 0 "透過第 0 號音訊輸出裝置播放"

# 從檔案或 stdin 管線朗讀
say -f article.txt
echo "Hello from stdin pipe!" | say -v en-US-AriaNeural

# TTY 互動模式（逐行輸入即時朗讀，按 Ctrl+D / Ctrl+Z 結束）
say

# 顯示進度條與逐字高亮朗讀
say --progress -i "你好世界，測試終端機進度條與逐字高亮同步顯示"
say --interactive=green "Highlighting spoken words in green."
```

## 支援平台

本套件提供以下 6 個平台的預編譯原生二進位檔：

| Node `platform-arch` | Rust Target | 壓縮格式 |
| --- | --- | --- |
| `darwin-arm64` | `aarch64-apple-darwin` | `.tar.xz` |
| `darwin-x64` | `x86_64-apple-darwin` | `.tar.xz` |
| `linux-x64` | `x86_64-unknown-linux-gnu` | `.tar.xz` |
| `linux-arm64` | `aarch64-unknown-linux-gnu` | `.tar.xz` |
| `win32-x64` | `x86_64-pc-windows-msvc` | `.zip` |
| `win32-arm64` | `aarch64-pc-windows-msvc` | `.zip` |

若在清單以外的平台使用，可安裝 Rust 工具鏈後從原始碼編譯。Linux 預編譯版需要 glibc 2.35 以上（例如 Ubuntu 22.04、Debian 12 以後）；更舊的發行版請從原始碼編譯。

## 本機開發與測試

### 系統相依套件（Linux 建置）

在 Linux 上從原始碼編譯需要 ALSA 開發標頭檔與 `pkg-config`：

```bash
sudo apt-get update && sudo apt-get install -y libasound2-dev pkg-config
```

### 建置與測試指令

```bash
# 檢查格式、靜態分析與 Rust 測試
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test

# 編譯本機 Release 二進位檔並以本機回退模式安裝至 npm 包裝器
cargo build --release
node npm/postinstall.cjs
node npm/cli.cjs --help

# 執行 npm 包裝器測試與薄型套件乾跑檢查
npm test
npm pack --dry-run
```

## 發佈流程（GitHub Actions）

本專案採用自動化 Release 與 npm 發佈管線。原始碼儲存庫是私有的，GitHub Releases 無法匿名下載，所以執行檔改由官方網站對外提供：

1. 同步更新 `Cargo.toml`、`Cargo.lock` 與 `package.json` 版本號並提交變更。
2. 推送版本標籤（例如 `git tag v0.1.1 && git push origin v0.1.1`）：
   - [`.github/workflows/release.yml`](.github/workflows/release.yml) 會於 6 個原生平台 runner 上編譯 `say`（Linux 版在 Ubuntu 22.04 上編譯，並檢查所需的 glibc 不超過 2.35），打包為 `say-<target>.tar.xz` / `.zip` 並產生對應 `.sha256` 校驗檔，最後建立 GitHub Release 並上傳全部 12 個資產。
3. Release 完成後：
   - [`.github/workflows/pages.yml`](.github/workflows/pages.yml) 會重新部署網站，把所有版本的 Release 資產鏡像到 `https://say.gh.miniasp.com/releases/download/<tag>/`，並以 `releases/latest.txt` 記錄最新版本。安裝腳本與 npm 套件都從這裡下載。
4. 網站部署完成後：
   - [`.github/workflows/npm-publish.yml`](.github/workflows/npm-publish.yml) 會取出最新 Release 的標籤、對齊版本，執行 `prepublishOnly`（`npm test`、`npm pack --dry-run` 以及 `node npm/prepublish-check.cjs` 驗證網站上 6 平台共 12 個資產皆可下載），接著以 `npm publish --access public` 發佈 `@willh/say`。已發佈過的版本會自動略過；也可以用 `gh workflow run npm-publish.yml` 手動執行。私有儲存庫無法產生 npm provenance，所以發佈時不帶 `--provenance`。

## 官方網站

[`site/`](site/) 是不需建置的純靜態網站（HTML、CSS、JavaScript），試聽音檔皆由本工具實際合成。推送到 `main` 且變更了 `site/**` 時，[`.github/workflows/pages.yml`](.github/workflows/pages.yml) 會自動部署到 GitHub Pages：<https://say.gh.miniasp.com/>。部署時也會把根目錄的 `install.sh`、`install.ps1` 複製到網站根目錄（修改這兩支腳本同樣會觸發部署）、把 Release 資產鏡像到 `releases/`，並以 [`scripts/stamp-assets.mjs`](scripts/stamp-assets.mjs) 替資源網址加上內容雜湊，避免瀏覽器快取到舊的 CSS 或 JavaScript。

本機預覽（需透過 HTTP 開啟，音訊分析與時間軸才能運作）：

```bash
python -m http.server 8787 --directory site
```

## 授權條款

採用 [MIT License](LICENSE) 授權。
