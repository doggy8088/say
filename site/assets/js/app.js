/* say website: bilingual copy, a headline that speaks like `say -i`, voice samples with live level meters. */
(() => {
  'use strict';

  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  /* ---------------- Copy ---------------- */
  const ZH = {
    heroPlay: '播放這句',
    stop: '停止',
    copy: '複製',
    copied: '已複製',
    playVoice: '播放 {voice} 的試聽',
    title: 'say：讓終端機開口說話的跨平台 TTS 指令',
    description: 'say 是跨平台的文字轉語音命令列工具，用法與 macOS 內建的 say 相容，可在 Windows、Linux、macOS 上執行，內建 300 多種 Microsoft Edge 神經語音，不需要安裝 ffmpeg。',
  };

  const EN = {
    title: 'say: a cross-platform text-to-speech command for your terminal',
    description: 'say is a cross-platform text-to-speech CLI compatible with macOS say. It runs on Windows, Linux and macOS with 300+ Microsoft Edge neural voices, and needs no ffmpeg.',
    skip: 'Skip to main content',
    brandLabel: 'say home',
    navLabel: 'Page sections',
    navVoices: 'Voices',
    navMacos: 'macOS',
    navInstall: 'Install',
    navUsage: 'Usage',
    navOptions: 'Options',
    ghLabel: 'View source on GitHub',
    heroPlay: 'Play this line',
    stop: 'Stop',
    heroCmd: 'say -v Ava -i "Give your terminal a voice."',
    heroLead: '<strong>say</strong> is a cross-platform text-to-speech command that works like the one built into macOS. It runs on Windows, Linux and macOS, comes with 300+ Microsoft Edge neural voices, and needs no ffmpeg.',
    copy: 'Copy',
    copied: 'Copied',
    heroNote: 'Requires Node.js 18 or later, and verifies the binary’s <span class="nw">SHA-256</span>. No Node.js? <a href="#install">Use the one-line installer</a>.',
    voicesTitle: '300+ voices. Start with these 8.',
    voicesLead: 'The default is HsiaoChen, a Taiwanese Mandarin voice. To switch, type any part of a voice name after <code>-v</code>; case doesn’t matter.',
    langZhTW: 'Mandarin (Taiwan)',
    langEnUS: 'English (US)',
    langJa: 'Japanese',
    langKo: 'Korean',
    langFr: 'French',
    default: 'Default',
    female: 'Female',
    male: 'Male',
    playVoice: 'Play the {voice} sample',
    voicesList: 'List every voice with <code>-v "?"</code>, and add <code>--locale</code> to narrow it to one language:',
    macosTitle: 'Coming from macOS? Your commands still work.',
    macosLead: 'Flags and behaviour follow the say that ships with macOS, so the scripts you already have run on Windows and Linux as they are. The main difference: voices come from Microsoft Edge’s online service, so say needs a network connection.',
    colFeature: 'Feature',
    fSpeak: 'Speak text given as arguments',
    fFile: 'Read from a file or stdin',
    fVoice: 'Choose a voice',
    nVoice: 'Partial names work',
    fRate: 'Speech rate',
    nRate: 'WPM or percent',
    fOut: 'Save to an audio file',
    fFormat: 'Set file and data formats',
    fDevice: 'Choose an output device',
    fProgress: 'Show progress',
    fInteractive: 'Highlight words as they’re spoken',
    fPitch: 'Adjust pitch and volume',
    fLocale: 'List voices by locale',
    fNetwork: 'Send audio to a network address',
    fChannels: 'Set channels, bit rate, quality',
    fOffline: 'Works offline',
    nOffline: 'Needs a network',
    installTitle: 'Install say',
    installLead: 'No Node.js? Use the one-line installer. Already have Node.js? npm works too. Either way you get a prebuilt binary for your system with its <span class="nw">SHA-256</span> verified, and you need neither Rust nor ffmpeg.',
    installTabs: 'Ways to install',
    tabScript: 'One-line install',
    scriptNote: 'Installs to <code>~/.local/bin</code>. On Windows the script adds that folder to your <code>PATH</code>; on macOS and Linux it prints the line to add if it isn’t there yet. To install somewhere else, set <code>SAY_INSTALL_DIR</code> first.',
    scriptUnix: 'macOS and Linux',
    scriptWin: 'Windows (PowerShell)',
    tabNpm: 'npm (global)',
    tabNpx: 'npx (no install)',
    tabCargo: 'Build from source',
    tabDl: 'Download a binary',
    cHello: 'say "Hello, world"',
    cNpx: 'npx @willh/say "Hello from cross-platform say!"',
    cAlsa: '# On Linux, install the ALSA headers first',
    dlText: 'Download the latest archive directly. Add <code>.sha256</code> to its URL for the checksum. Unpack it and put <code>say</code> in any folder on your <code>PATH</code>.',
    platformsTitle: 'Prebuilt platforms',
    pAppleSilicon: 'Apple silicon',
    platformsNote: 'The Linux builds need <span class="nw">glibc 2.35</span> or later, such as <span class="nw">Ubuntu 22.04</span> or <span class="nw">Debian 12</span>. On anything else, install Rust and build from source.',
    usageTitle: 'Everyday commands',
    usageLead: 'From a single sentence to a spoken heads-up when your build is done.',
    rSpeakT: 'Speak some text',
    rSpeakP: 'Multiple arguments are joined with spaces. Give none, and say reads each line you type aloud; press Ctrl+D to finish.',
    rSpeakC: '<span class="pr">$</span> say "Good morning! Lovely day today."\n<span class="pr">$</span> say Hello from cross-platform say',
    rBuildT: 'Hear when a build finishes',
    rBuildP: 'Chain say after a long-running command and stop watching the screen.',
    rBuildC: '<span class="pr">$</span> npm run build &amp;&amp; say "Build done" || say "Build failed"\n<span class="pr">$</span> git log <span class="fl">-1</span> <span class="fl">--pretty</span>=%s | say',
    rVoiceT: 'Use a different voice',
    rVoiceP: 'Full names and partial names both work. Set <code>SAY_VOICE</code> to change the default.',
    rVoiceC: '<span class="pr">$</span> say <span class="fl">-v</span> Jenny "Welcome to say"\n<span class="pr">$</span> SAY_VOICE=en-US-JennyNeural say "Hello there"',
    rRateT: 'Change the speed',
    rRateP: 'Values with <code>%</code> or a sign are percentages; a bare number is a macOS WPM rate, where 175 WPM equals <code>+0%</code>. Drag to see how the two line up:',
    rRateLabel: 'macOS rate',
    rRateText1: '"A little faster"',
    rRateText2: '"Exactly the same speed"',
    rPitchT: 'Change pitch and volume',
    rPitchP: 'Pitch is in Hz, volume in percent; both can go negative.',
    rPitchC: '<span class="pr">$</span> say <span class="fl">--pitch</span> +10Hz <span class="fl">--volume</span> +15% "Higher and louder"\n<span class="pr">$</span> say <span class="fl">--pitch</span> -8Hz <span class="fl">--volume</span> -30% "Lower and quieter"',
    rSaveT: 'Save to an audio file',
    rSaveP: 'Add <code>-o</code> to write a file instead of playing. The extension picks the format: mp3, wav, ogg or webm.',
    rSaveC: '<span class="pr">$</span> say <span class="fl">-o</span> greeting.mp3 "Saved as MP3"\n<span class="pr">$</span> say <span class="fl">-o</span> notice.wav <span class="fl">-r</span> +10% "Saved as WAV"',
    rPipeT: 'Read from a file or a pipe',
    rPipeP: 'Long text is split into chunks, synthesised, and joined back into one clip.',
    rDeviceT: 'Pick an output device',
    rDeviceP: 'List the devices, then choose one by number or by the start of its name.',
    rDeviceC: '<span class="pr">$</span> say <span class="fl">-a</span> "?"\n<span class="pr">$</span> say <span class="fl">-a</span> 0 "Playing on device 0"',
    rHiT: 'Highlight as it speaks',
    rHiP: '<code>-i</code> marks each word in your terminal as it’s spoken, in reverse video or a colour you choose. <code>--progress</code> shows a progress bar.',
    rHiC: '<span class="pr">$</span> say <span class="fl">-i</span> "Reverse video by default"\n<span class="pr">$</span> say <span class="fl">--interactive=green</span> "Now in green"\n<span class="pr">$</span> say <span class="fl">--progress</span> "Showing synthesis and playback progress"',
    agentsTitle: 'A note for your agent',
    agentsLead: 'Paste this into <code>AGENTS.md</code>, <code>CLAUDE.md</code> or your agent’s custom instructions. Each time it finishes a task, it reads a short summary aloud with say, so you hear how it went even when you’re away from the screen. Any agent that can run terminal commands will do: <span class="nw">Claude Code</span>, Codex, Copilot, Cursor and others.',
    agentsPrompt: 'When the whole task is done, right before you reply to me, use `say` to read a short summary aloud:\n- One or two sentences: what you did, whether it worked, and anything I need to look at.\n- It will be spoken, so keep it conversational. Skip code, file paths and URLs.\n- Example: `say -v Ava "Fixed the login bug and all tests pass. Please check the new error message."`',
    optionsTitle: 'All options',
    optionsLead: 'Also available in your terminal with <code>say --help</code>.',
    oColOpt: 'Option',
    oColDef: 'Default',
    oColDesc: 'Description',
    oMsg: 'Text to speak; multiple arguments are joined with spaces. With no text and no <code>-f</code>, say reads from stdin.',
    oVoice: 'Voice name: a full ShortName or any part of one. The <code>SAY_VOICE</code> environment variable sets the default. Pass <code>?</code> to list all voices.',
    oList: 'List all voices, same as <code>-v "?"</code>.',
    oLocale: 'Filter the voice list by locale prefix, such as <code>zh-TW</code>, <code>en-US</code> or <code>ja</code>.',
    oRate: 'Speech rate. <code>+20%</code> or <code>-15%</code> is a percentage (-100 to 200); a bare number like <code>200</code> or <code>120</code> is WPM, where 175 WPM equals 0%.',
    oPitch: 'Pitch, such as <code>+10Hz</code> or <code>-5Hz</code>.',
    oVolume: 'Volume, such as <code>+20%</code> or <code>-30%</code>.',
    oOut: 'Write an audio file instead of playing; the extension picks the format.',
    oFileFmtDef: 'From extension',
    oFileFmt: '<code>mp3</code>, <code>wav</code>, <code>ogg</code> or <code>webm</code>. Pass <code>?</code> to list supported formats.',
    oDataFmt: 'A native Edge TTS <code>audio_format</code> string, such as <code>audio-24khz-96kbitrate-mono-mp3</code>. Pass <code>?</code> to list them.',
    oFile: 'Read text from a file; <code>-</code> means stdin.',
    oDeviceDef: 'System default',
    oDevice: 'Output device number or the start of its name (case-insensitive). Pass <code>?</code> to list devices.',
    off: 'Off',
    oProgress: 'Show synthesis and playback progress on stderr.',
    oInteractive: 'Mark each word in the terminal as it’s spoken. Accepts <code>bold</code>, <code>smul</code>, <code>smso</code>, a colour name, or a foreground/background pair like <code>yellow/blue</code>.',
    oHelp: 'Show help.',
    oVersion: 'Show the version.',
    closeTitle: 'Install it, then let it say hello.',
    cHi: 'say "Nice to meet you"',
    closeGh: 'View source on GitHub',
    closeNpm: 'View the npm package',
    brandTop: 'Back to top',
    footerNav: 'Related links',
    fReleases: 'Releases',
    fIssues: 'Report an issue',
  };

  const HERO = {
    zh: { clip: 'hero-zh', block: true },
    en: { clip: 'hero-en', breakAfter: 2 }, // "Give your terminal" / "a voice."
  };

  let lang = 'zh';
  const t = (key) => (lang === 'en' ? EN[key] : ZH[key]) ?? key;

  /* ---------------- i18n ---------------- */
  const textNodes = $$('[data-i18n]');
  const zhText = new Map(textNodes.map((n) => [n, n.innerHTML]));
  const ariaNodes = $$('[data-i18n-aria]');
  const zhAria = new Map(ariaNodes.map((n) => [n, n.getAttribute('aria-label')]));
  const metaDesc = $('meta[name="description"]');
  const toggle = $('#lang-toggle');

  function applyLang(next) {
    lang = next === 'en' ? 'en' : 'zh';
    stop();
    cancelTyping();
    document.documentElement.lang = lang === 'en' ? 'en' : 'zh-Hant-TW';
    for (const n of textNodes) {
      const key = n.dataset.i18n;
      n.innerHTML = lang === 'en' && key in EN ? EN[key] : zhText.get(n);
      for (const code of n.querySelectorAll('code')) code.translate = false;
    }
    for (const n of ariaNodes) {
      const key = n.dataset.i18nAria;
      let label = lang === 'en' && key in EN ? EN[key] : zhAria.get(n);
      if (key === 'playVoice') label = t('playVoice').replace('{voice}', n.dataset.voice);
      n.setAttribute('aria-label', label);
    }
    document.title = t('title');
    metaDesc.setAttribute('content', t('description'));
    toggle.textContent = lang === 'en' ? '中文' : 'EN';
    toggle.lang = lang === 'en' ? 'zh-Hant-TW' : 'en';
    toggle.setAttribute('aria-label', lang === 'en' ? '切換為中文' : 'Switch to English');
    try { localStorage.setItem('say-lang', lang); } catch { /* storage unavailable */ }
    // Keep a shareable URL: ?lang=en for English, no parameter for the default Chinese page.
    const url = new URL(location.href);
    if (lang === 'en') url.searchParams.set('lang', 'en'); else url.searchParams.delete('lang');
    history.replaceState(history.state, '', url);
    buildHero();
  }

  function initialLang() {
    const param = new URLSearchParams(location.search).get('lang');
    if (param === 'en' || param === 'zh') return param;
    try {
      const saved = localStorage.getItem('say-lang');
      if (saved === 'zh' || saved === 'en') return saved;
    } catch { /* storage unavailable */ }
    const langs = navigator.languages?.length ? navigator.languages : [navigator.language || 'zh'];
    return langs.some((l) => /^zh/i.test(l)) ? 'zh' : 'en';
  }

  toggle.addEventListener('click', () => applyLang(lang === 'en' ? 'zh' : 'en'));

  /* ---------------- Speaking text ---------------- */
  let TIMINGS = null;
  const PUNCT = /^[\p{P}\p{Z}\s]+$/u;

  function tokenize(text) {
    if (/\s/.test(text.trim())) return text.match(/\S+\s*/g) || [text];
    const out = [];
    for (const ch of Array.from(text)) {
      if (PUNCT.test(ch) && out.length) out[out.length - 1] += ch;
      else out.push(ch);
    }
    return out;
  }
  const weight = (tok) => (tok.match(/[\p{L}\p{N}]/gu) || []).length;

  // Render a clip's phrases as spans that each know when they are spoken.
  function renderSpeak(el, clipId, opts = {}) {
    const clip = TIMINGS?.[clipId];
    if (!clip) return;
    el.textContent = '';
    const tokens = [];
    let index = 0;
    for (const seg of clip.segs) {
      const holder = opts.block ? document.createElement('span') : el;
      if (opts.block) { holder.className = 'seg'; el.append(holder); }
      const parts = tokenize(seg.t);
      const total = parts.reduce((n, p) => n + weight(p), 0) || 1;
      let acc = seg.s;
      for (const part of parts) {
        const span = document.createElement('span');
        span.className = 'tk';
        span.textContent = part;
        const len = ((seg.e - seg.s) * weight(part)) / total;
        tokens.push({ el: span, s: acc, e: acc + len });
        acc += len;
        holder.append(span);
        if (opts.breakAfter === index) holder.append(document.createElement('br'));
        index += 1;
      }
    }
    el._tokens = tokens;
  }

  function paint(el, time) {
    const tokens = el?._tokens;
    if (!tokens) return;
    let now = -1;
    for (let i = 0; i < tokens.length; i += 1) if (tokens[i].s <= time) now = i;
    tokens.forEach((tok, i) => {
      tok.el.classList.toggle('is-done', i < now);
      tok.el.classList.toggle('is-now', i === now);
    });
  }

  function clearPaint(el) {
    el?._tokens?.forEach((tok) => tok.el.classList.remove('is-done', 'is-now'));
  }

  const heroLine = $('#hero-line');
  function buildHero() {
    const cfg = HERO[lang];
    heroLine.dataset.clip = cfg.clip;
    if (TIMINGS) renderSpeak(heroLine, cfg.clip, cfg);
  }

  /* ---------------- Audio and meters ---------------- */
  const audio = new Audio();
  audio.preload = 'auto';
  let ctx = null;
  let analyser = null;
  let bins = null;
  let current = null;
  let raf = 0;

  function ensureGraph() {
    if (ctx) {
      if (ctx.state === 'suspended') ctx.resume();
      return;
    }
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return;
    try {
      ctx = new AC();
      const source = ctx.createMediaElementSource(audio);
      analyser = ctx.createAnalyser();
      analyser.fftSize = 256;
      analyser.smoothingTimeConstant = 0.55;
      source.connect(analyser);
      analyser.connect(ctx.destination);
      bins = new Uint8Array(analyser.frequencyBinCount);
    } catch {
      analyser = null;
    }
  }

  const BAR_COUNT = 24;
  for (const meter of $$('.meter')) {
    for (let i = 0; i < BAR_COUNT; i += 1) meter.append(document.createElement('i'));
  }
  // Spread the bars over roughly 150 Hz to 7.5 kHz, denser at the low end where speech lives.
  const bandEdges = Array.from({ length: BAR_COUNT + 1 }, (_, i) => 1 + Math.round(40 * (i / BAR_COUNT) ** 1.6));

  function drawMeter(bars) {
    if (!analyser || !bars) return;
    analyser.getByteFrequencyData(bins);
    bars.forEach((bar, i) => {
      let peak = 0;
      for (let b = bandEdges[i]; b <= Math.max(bandEdges[i], bandEdges[i + 1] - 1); b += 1) peak = Math.max(peak, bins[b]);
      const v = peak / 255;
      bar.style.transform = `scaleY(${(0.1 + 0.9 * v ** 1.35).toFixed(3)})`;
    });
  }

  function loop() {
    if (!current) return;
    paint(current.speak, audio.currentTime);
    drawMeter(current.bars);
    raf = requestAnimationFrame(loop);
  }

  function setHeroLabel(playing) {
    const label = $('.btn--play .btn__label');
    if (label) label.textContent = playing ? t('stop') : t('heroPlay');
  }

  function stop() {
    if (!current) return;
    audio.pause();
    cancelAnimationFrame(raf);
    const { btn, strip, speak, bars, isHero } = current;
    btn.setAttribute('aria-pressed', 'false');
    strip?.classList.remove('is-active');
    speak?.classList.remove('is-speaking');
    clearPaint(speak);
    bars?.forEach((b) => { b.style.transform = ''; });
    if (isHero) setHeroLabel(false);
    current = null;
  }

  function start(btn) {
    const id = btn.dataset.play;
    const isHero = id === 'hero';
    const clip = isHero ? heroLine.dataset.clip : id;
    if (current && current.btn === btn) { stop(); return; }
    stop();
    ensureGraph();
    const strip = btn.closest('.strip');
    current = {
      btn,
      isHero,
      strip,
      speak: isHero ? heroLine : strip?.querySelector('.speak'),
      bars: strip ? $$('.meter i', strip) : null,
    };
    btn.setAttribute('aria-pressed', 'true');
    strip?.classList.add('is-active');
    current.speak?.classList.add('is-speaking');
    if (isHero) setHeroLabel(true);
    audio.src = `assets/audio/${clip}.mp3`;
    audio.play().then(loop).catch(stop);
  }

  audio.addEventListener('ended', stop);
  for (const btn of $$('[data-play]')) btn.addEventListener('click', () => start(btn));
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape') stop(); });

  /* ---------------- Copy buttons ---------------- */
  const live = $('#live');
  const commandsIn = (text) => {
    const lines = text.split('\n').filter((l) => l.startsWith('$ ')).map((l) => l.slice(2));
    return lines.length ? lines.join('\n') : text.trim();
  };

  async function writeClipboard(text) {
    if (navigator.clipboard?.writeText) return navigator.clipboard.writeText(text);
    const area = Object.assign(document.createElement('textarea'), { value: text });
    area.setAttribute('readonly', '');
    area.style.cssText = 'position:fixed;opacity:0';
    document.body.append(area);
    area.select();
    document.execCommand('copy');
    area.remove();
  }

  document.addEventListener('click', async (e) => {
    const btn = e.target.closest('.copy');
    if (!btn) return;
    // data-copy-all copies the whole block as shown (a prose prompt); otherwise only the `$ ` command lines.
    const shown = btn.closest('.term')?.querySelector('pre code')?.textContent || '';
    const text = btn.dataset.copy || (btn.hasAttribute('data-copy-all') ? shown.trim() : commandsIn(shown));
    try {
      await writeClipboard(text);
      btn.textContent = t('copied');
      btn.classList.add('is-done');
      live.textContent = t('copied');
      clearTimeout(btn._timer);
      btn._timer = setTimeout(() => {
        btn.textContent = t('copy');
        btn.classList.remove('is-done');
      }, 1600);
    } catch { /* clipboard blocked: leave the text selectable */ }
  });

  /* ---------------- Tabs ---------------- */
  for (const box of $$('[data-tabs]')) {
    const tabs = $$('[role="tab"]', box);
    const select = (tab, focus) => {
      for (const other of tabs) {
        const on = other === tab;
        other.setAttribute('aria-selected', String(on));
        other.tabIndex = on ? 0 : -1;
        document.getElementById(other.getAttribute('aria-controls')).hidden = !on;
      }
      if (focus) tab.focus();
    };
    // Panels are all visible in the markup so the page works without JavaScript.
    select(tabs.find((tab) => tab.getAttribute('aria-selected') === 'true') || tabs[0], false);
    tabs.forEach((tab, i) => {
      tab.addEventListener('click', () => select(tab, false));
      tab.addEventListener('keydown', (e) => {
        const step = { ArrowRight: 1, ArrowLeft: -1 }[e.key];
        if (step) { e.preventDefault(); select(tabs[(i + step + tabs.length) % tabs.length], true); }
        if (e.key === 'Home') { e.preventDefault(); select(tabs[0], true); }
        if (e.key === 'End') { e.preventDefault(); select(tabs[tabs.length - 1], true); }
      });
    });
  }

  /* ---------------- Rate: WPM ↔ percent ---------------- */
  const wpm = $('#wpm');
  if (wpm) {
    const out = $('#wpm-out');
    const rw = $('#rate-wpm');
    const rp = $('#rate-pct');
    const update = () => {
      const v = Number(wpm.value);
      const pct = Math.max(-100, Math.min(200, Math.round(((v - 175) / 175) * 100)));
      out.textContent = `${v} WPM`;
      rw.textContent = String(v);
      rp.textContent = `${pct >= 0 ? '+' : ''}${pct}%`;
    };
    wpm.addEventListener('input', update);
    update();
  }

  /* ---------------- Nav state ---------------- */
  const nav = $('#nav');
  const onScroll = () => nav.classList.toggle('is-scrolled', window.scrollY > 8);
  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();

  /* ---------------- Page-load moment: the command types itself ---------------- */
  let typingTimer = 0;
  function typeCommand() {
    const cmd = $('#hero-cmd');
    const text = $('.hero__cmd-text', cmd);
    if (reduceMotion || !text) return;
    const chars = Array.from(text.textContent);
    text.textContent = '';
    cmd.classList.add('is-typing');
    let i = 0;
    const tick = () => {
      i += 1;
      text.textContent = chars.slice(0, i).join('');
      if (i < chars.length) typingTimer = setTimeout(tick, 34);
      else { cmd.classList.remove('is-typing'); cmd.classList.add('is-caret'); }
    };
    typingTimer = setTimeout(tick, 350);
  }
  function cancelTyping() {
    clearTimeout(typingTimer);
    $('#hero-cmd')?.classList.remove('is-typing');
  }

  /* ---------------- Boot ---------------- */
  applyLang(initialLang());
  typeCommand();

  fetch('assets/audio/timings.json')
    .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
    .then((data) => {
      TIMINGS = data;
      buildHero();
      for (const strip of $$('.strip')) {
        const clip = strip.dataset.clip;
        renderSpeak($('.speak', strip), clip);
        // At rest, each meter shows the clip's own loudness envelope.
        const env = TIMINGS[clip]?.env || [];
        $$('.meter i', strip).forEach((bar, i) => bar.style.setProperty('--rest', env[i] ?? 0.1));
      }
    })
    .catch(() => { /* highlighting is optional; audio still plays */ });
})();
