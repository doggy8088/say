.PHONY: help build release install-local uninstall-local stage-npm fmt fmt-check lint test-rust test-npm test pack-dry-run check prepublish-check voices devices demo run clean

RUN = @node -e "const {spawnSync}=require('child_process');const a=process.argv.slice(1);const r=spawnSync(a[0],a.slice(1),{stdio:'inherit',shell:process.platform==='win32'});process.exit(r.status===null?1:r.status);" --

help: ## 顯示所有常用任務與繁體中文說明
	@node -e "const fs=require('fs');console.log('say CLI 常用開發任務 (make <目標>)：\n');for(const l of fs.readFileSync('Makefile','utf8').split(/\r?\n/)){const i=l.indexOf('## ');const c=l.indexOf(':');if(i>0&&c>0&&c<i){const k=l.slice(0,c).trim();if(k&&!k.includes(' ')&&!k.startsWith('.'))console.log('  '+k.padEnd(18)+l.slice(i+3).trim());}}"

build: ## 編譯開發版 (Debug) Rust 執行檔
	@node -e "console.log('==> 正在編譯開發版 (Debug) Rust 執行檔...');"
	$(RUN) cargo build

release: ## 編譯正式版 (Release) Rust 執行檔
	@node -e "console.log('==> 正在編譯正式版 (Release) Rust 執行檔...');"
	$(RUN) cargo build --release

install-local: release ## 編譯 Release 並安裝 say 執行檔至 ~/.local/bin
	@node -e "const fs=require('fs'),os=require('os'),path=require('path');const bin=process.platform==='win32'?'say.exe':'say';const src=path.join('target','release',bin);const dir=path.join(os.homedir(),'.local','bin');const dst=path.join(dir,bin);fs.mkdirSync(dir,{recursive:true});fs.copyFileSync(src,dst);fs.chmodSync(dst,0o755);console.log('==> 已安裝至 '+dst);console.log('==> 若無法直接執行 say，請確認 ~/.local/bin 已加入系統 PATH。');"

uninstall-local: ## 從 ~/.local/bin 移除本機安裝的 say 執行檔
	@node -e "const fs=require('fs'),os=require('os'),path=require('path');const bin=process.platform==='win32'?'say.exe':'say';const dst=path.join(os.homedir(),'.local','bin',bin);if(fs.existsSync(dst)){fs.rmSync(dst,{force:true});console.log('==> 已移除 '+dst);}else{console.log('==> 未找到 '+dst+'，無需移除。');}"

stage-npm: release ## 編譯 Release 並安裝至本機 npm/say-bin/
	@node -e "console.log('==> 正在安裝 Release 二進位檔至本機 npm/say-bin/...');"
	node npm/install.cjs
	@node -e "console.log('==> 本機 npm 包裝安裝完成，可使用 node npm/cli.cjs 執行。');"

fmt: ## 自動格式化全部 Rust 程式碼
	@node -e "console.log('==> 正在格式化 Rust 程式碼...');"
	$(RUN) cargo fmt --all

fmt-check: ## 檢查 Rust 程式碼格式是否符合規範
	@node -e "console.log('==> 正在檢查 Rust 程式碼格式...');"
	$(RUN) cargo fmt --all -- --check

lint: ## 執行 Clippy 靜態程式碼檢查 (零警告)
	@node -e "console.log('==> 正在執行 Clippy 靜態分析...');"
	$(RUN) cargo clippy --all-targets -- -D warnings

test-rust: ## 執行全部 Rust 單元與整合測試
	@node -e "console.log('==> 正在執行 Rust 測試...');"
	$(RUN) cargo test

test-npm: ## 執行 npm 包裝輔助函式 Node 測試
	@node -e "console.log('==> 正在執行 npm 包裝 Node 測試...');"
	$(RUN) npm test

test: test-rust test-npm ## 執行全部 Rust 與 Node 測試
	@node -e "console.log('==> 所有 Rust 與 Node 測試皆已通過。');"

pack-dry-run: ## 乾跑檢查 npm 打包檔案清單
	@node -e "console.log('==> 正在乾跑檢查 npm 打包內容...');"
	$(RUN) npm pack --dry-run

check: fmt-check lint test pack-dry-run ## 執行完整 CI 品質檢查流程 (格式、Clippy、測試、打包乾跑)
	@node -e "console.log('==> 完整 CI 品質檢查流程全數通過！');"

prepublish-check: ## 檢查 GitHub Releases 遠端產物與 SHA-256 校驗檔
	@node -e "console.log('==> 正在檢查 GitHub Releases 遠端產物...');"
	node npm/prepublish-check.cjs

voices: ## 透過本機包裝查詢台灣中文可用語音
	@node -e "console.log('==> 正在查詢台灣中文 (zh-TW) 可用語音清單...');"
	@node -e "const {spawnSync}=require('child_process');const r=spawnSync(process.execPath,['npm/cli.cjs','-v','?','--locale','zh-TW'],{stdio:'inherit'});process.exit(r.status===null?1:r.status);"

devices: ## 透過本機包裝列出本機音訊輸出裝置
	@node -e "console.log('==> 正在列出本機音訊輸出裝置...');"
	@node -e "const {spawnSync}=require('child_process');const r=spawnSync(process.execPath,['npm/cli.cjs','-a','?'],{stdio:'inherit'});process.exit(r.status===null?1:r.status);"

demo: ## 透過本機包裝試聽繁體中文示範語音 (可用 MSG 環境變數覆寫內容)
	@node -e "const {spawnSync}=require('child_process');const msg=process.env.MSG||'你好世界，這是跨平台的 say 語音工具';console.log('==> 正在透過本機包裝試聽示範語音...');const r=spawnSync(process.execPath,['npm/cli.cjs','--progress','-i',msg],{stdio:'inherit'});process.exit(r.status===null?1:r.status);"

run: ## 執行開發版 CLI (可搭配 ARGS='...' 傳入參數)
	@node -e "console.log('==> 正在執行開發版 say CLI...');"
	$(RUN) cargo run -- $(ARGS)

clean: ## 清理 Cargo 建置產物、本機安裝二進位檔與暫存音訊檔
	@node -e "console.log('==> 正在清理建置產物與暫存檔...');"
	$(RUN) cargo clean
	@node -e "const fs=require('fs');fs.rmSync('npm/say-bin',{recursive:true,force:true});for(const f of fs.readdirSync('.')){if(f.startsWith('_verify_')&&(f.endsWith('.mp3')||f.endsWith('.wav')||f.endsWith('.ogg')||f.endsWith('.webm')))fs.rmSync(f,{force:true});}console.log('==> 清理完成。');"
