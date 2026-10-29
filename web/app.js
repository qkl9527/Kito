const icons = {
  grid: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>',
  image:
    '<rect x="3" y="3" width="18" height="18" rx="3"/><circle cx="8" cy="8" r="1.5"/><path d="m3 16 5-5 5 6 3-3 5 5"/>',
  time: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l4 2"/>',
  codec: '<path d="m8 6-6 6 6 6m8-12 6 6-6 6m-3-14-2 16"/>',
  id: '<path d="M9 3 7 21M17 3l-2 18M3 9h18M2 15h18"/>',
  key: '<circle cx="8" cy="8" r="5"/><path d="m12 12 9 9m-5-5 3-3m-1 5 3-3"/>',
  video:
    '<rect x="3" y="4" width="18" height="16" rx="3"/><path d="m10 8 6 4-6 4z"/>',
  sound: '<path d="M3 10v4m4-8v12m5-16v20m5-16v12m4-8v4"/>',
  text: '<path d="M3 5h12M9 5v15m6-8h7m-4-3v11"/>',
  search: '<circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/>',
  arrow: '<path d="M4 12h16m-6-6 6 6-6 6"/>',
  "arrow-up-right": '<path d="M6 18 18 6M7 6h11v11"/>',
  close: '<path d="m6 6 12 12M6 18 18 6"/>',
  shield:
    '<path d="m12 3 8 3v6c0 5-8 9-8 9s-8-4-8-9V6z"/><path d="m8 12 3 3 5-6"/>',
  check: '<path d="m5 12 4 4L19 6"/>',
  monitor:
    '<rect x="3" y="3" width="18" height="13" rx="2"/><path d="M8 21h8m-4-5v5"/>',
  book: '<path d="M12 5c-3-2-7-2-10-1v15c3-1 7-1 10 1 3-2 7-2 10-1V4c-3-1-7-1-10 1zm0 0v15"/>',
  upload: '<path d="M12 16V3m-5 5 5-5 5 5M4 15v5h16v-5"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6m0-10v1"/>',
  terminal: '<path d="m4 6 6 6-6 6m9 0h7"/>',
  menu: '<path d="M4 6h16M4 12h16M4 18h16"/>',
  replay: '<path d="M3 10a9 9 0 1 1 1 7M3 4v6h6"/>',
};
const icon = (name) =>
  `<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${icons[name] || icons.grid}</svg>`;
const tools = [
  {
    id: "image",
    name: "图片处理",
    partner: "Lumi 露米",
    title: "光影师",
    asset: "lumi",
    description: "压缩 PNG、转换格式、调整尺寸",
    tone: "#fff4d6",
    ink: "#9b7823",
    dot: "#d6ad45",
    abilities: [],
  },
  {
    id: "time",
    name: "时间转换",
    partner: "Tiko 提可",
    title: "时刻向导",
    asset: "tiko",
    description: "时间戳转换与全球时区换算",
    tone: "#e6f2ec",
    ink: "#50836b",
    dot: "#87b49b",
    abilities: [
      [
        "时间戳互转",
        "在 Unix 时间戳与日期时间之间转换，选择秒、毫秒、微秒或纳秒。",
      ],
      ["跨时区转换", "选定来源和目标时区，把同一个时刻换算清楚。"],
      ["全球时区查询", "按地区、名称或偏移查找时区，偏移与参考时刻一起展示。"],
      ["默认时区", "保存常用时区，减少重复选择。"],
    ],
    boundary:
      "夏令时带来的歧义或不存在的时间会明确提示；降低时间戳精度时说明取整。",
  },
  {
    id: "codec",
    name: "编解码",
    partner: "Codi 可迪",
    title: "译码师",
    asset: "codi",
    description: "Base64、URL 参数与 HTML 实体",
    tone: "#efebf9",
    ink: "#80709f",
    dot: "#af9bce",
    abilities: [
      [
        "Base64 编解码",
        "支持标准与 URL-safe 字母表，把文本或文件转换成需要的表示。",
      ],
      ["URL 参数解析", "清楚列出每个参数，保留重复参数、空值与原始次序。"],
      ["HTML / XML 实体", "编码或解码 HTML5 与 XML 1.0 实体，一次只处理一层。"],
    ],
    boundary: "编码不等于加密；HTML 实体转换也不用于清洗不安全的 HTML。",
  },
  {
    id: "id",
    name: "指纹与标识",
    partner: "Sumi 苏米",
    title: "印记师",
    asset: "sumi",
    description: "计算内容 Hash，生成 UUID",
    tone: "#f9eae2",
    ink: "#a1735a",
    dot: "#d49b7d",
    abilities: [
      ["计算 Hash", "按文本或文件的真实字节计算摘要，支持六种算法。"],
      ["生成 UUID", "选择 UUID v4 或 v7，为对象生成标识。"],
    ],
    boundary: "Hash 是内容指纹，UUID 是对象标识；UUID 不能替代密钥或授权凭证。",
  },
  {
    id: "key",
    name: "密钥工具",
    partner: "Kavi 卡维",
    title: "钥匙匠",
    asset: "kavi",
    description: "生成密钥对，导出与检查公钥",
    tone: "#e9f0fa",
    ink: "#5e7d9e",
    dot: "#8daecb",
    abilities: [
      [
        "生成密钥对",
        "使用 OpenSSL 3.x 生成 RSA 或 Ed25519 密钥，保存到新的独立目录。",
      ],
      ["导出公钥", "从私钥中导出 SPKI 格式的公钥。"],
      ["检查配对", "确认私钥与公钥是否匹配，并检查后端状态。"],
    ],
    boundary:
      "Ed25519 用于签名。私钥和口令不进入报告；私钥仅供下载，口令在本次请求后立即清空。",
  },
  {
    id: "video",
    name: "视频与音频",
    partner: "Vido 维多",
    title: "剪影师",
    asset: "vido",
    description: "剪辑与转换视频，处理现有音频",
    tone: "#f8e8eb",
    ink: "#a26d78",
    dot: "#d699a2",
    abilities: [
      [
        "转换与压缩",
        "将本地视频转换为下一步使用的格式，也可以调整尺寸与播放速度。",
      ],
      ["剪辑与画面", "裁剪片段、裁切画面、旋转、拼接，或去除音轨。"],
      ["现有音频处理", "提取音轨，转换音乐格式，剪辑、混音、淡化或调整响度。"],
      ["截图与 GIF", "从本地视频截取 PNG，或生成短小的 GIF。"],
    ],
    boundary:
      "视频重新编码有损，无法保证文件更小。字幕、附件和原有元数据不保留；实际执行依赖本机 FFmpeg / ffprobe。",
  },
  {
    id: "sound",
    name: "声音创作",
    partner: "Echo 艾可",
    title: "音效师",
    asset: "echo",
    description: "从零制作音效、旋律与鼓点",
    tone: "#edecf9",
    ink: "#76759f",
    dot: "#a7a1c7",
    abilities: [
      ["一键音效", "选择场景，合成提示音与游戏音效。"],
      [
        "单音与旋律",
        "用音名或频率生成单音，用和弦与休止编写旋律。默认输出 WAV，旋律可另存 MIDI。",
      ],
      ["鼓点创作", "选择节奏预设，或用 16 格节奏编排自己的鼓点。"],
    ],
    boundary:
      "使用合成音色，不是真实乐器采样；MIDI 保存音符与节奏，音色由播放软件决定。现有音乐编辑请交给 Vido。",
  },
  {
    id: "text",
    name: "文本工具",
    partner: "日常小工具",
    title: "",
    asset: null,
    description: "字符统计、按行去重、大小写转换",
    tone: "#edf0f3",
    ink: "#6b7885",
    dot: "#a7b0b8",
    abilities: [
      ["字符统计", "统计 Unicode 字符数量或 UTF-8 字节数。"],
      ["按行去重", "保留第一次出现的行及其顺序，空行也参与去重。"],
      ["大小写转换", "转换大写、小写，或调整首字符与单词首字母。"],
    ],
    boundary:
      "输入文件必须是有效的 UTF-8 文本。字符计数按 Unicode scalar value，不等同于视觉上的字素簇。",
  },
];

// 表单仅能产生白名单动作；路径、程序名和任意 CLI 参数从不交给浏览器输入。
const field = (key, label, value = "", options = null, hint = "") => ({
  key,
  label,
  value,
  options,
  hint,
});
const action = (id, label, fields = [], input = "none", note = "") => ({
  id,
  label,
  fields,
  input,
  note,
});
const f = field;
const unit = () => f("unit", "时间戳单位", "s", ["s", "ms", "us", "ns"]);
const zone = (key = "timezone", label = "时区") =>
  f(key, label, "UTC", null, "IANA 时区，如 Asia/Shanghai；默认 UTC");
const format = () =>
  f("format", "日期展示格式", "", null, "可选 strftime，如 %Y-%m-%d %H:%M:%S");
const quality = () =>
  f("quality", "编码质量", "balanced", ["high", "balanced", "small"]);
const audio = () => f("to", "音频格式", "mp3", ["mp3", "m4a", "wav", "flac"]);
const instrument = () =>
  f("instrument", "合成音色", "sine", [
    "sine",
    "soft",
    "bell",
    "pluck",
    "chip",
  ]);
const actions = {
  image: [
    action(
      "compress",
      "压缩 PNG",
      [f("effort", "优化方式", "max", ["max", "fast"])],
      "image",
      "严格保持原始像素、透明像素 RGB 与显示相关元数据。压缩效果取决于原图。",
    ),
    action(
      "convert",
      "格式转换",
      [
        f("to", "目标格式", "png", ["png", "webp", "jpeg", "tiff", "avif"]),
        f("allow_lossy", "允许有损转换", "false", ["false", "true"]),
        f("quality", "有损质量（1–100）", ""),
      ],
      "image",
      "JPEG 等有损格式需明确允许有损；格式支持取决于本机实际编解码器。",
    ),
    action(
      "resize",
      "调整尺寸",
      [
        f(
          "resize",
          "目标尺寸",
          "50%",
          null,
          "如 50%、1280x720；默认等比，允许放大",
        ),
        f("to", "输出格式", "png", ["png", "webp", "jpeg", "tiff"]),
        f("allow_lossy", "允许有损转换", "false", ["false", "true"]),
      ],
      "image",
      "尺寸改变会重采样；无损验证仅针对缩放后的参考图。",
    ),
    action("info", "图片信息", [], "image"),
    action("exif", "查看元数据", [], "image"),
  ],
  time: [
    action("now", "当前时间", [zone(), unit(), format()]),
    action("from-timestamp", "时间戳转日期", [
      f("value", "时间戳", ""),
      f("unit", "输入单位", "auto", ["auto", "s", "ms", "us", "ns"]),
      zone(),
      format(),
    ]),
    action("to-timestamp", "日期转时间戳", [
      f("datetime", "日期时间", "", null, "如 2026-10-29T12:00:00+08:00"),
      unit(),
      f("timezone", "无偏移日期的源时区", ""),
      format(),
    ]),
    action("convert", "时区转换", [
      f("datetime", "日期时间", ""),
      f("from_timezone", "源时区（输入无偏移时填写）", ""),
      zone("to_timezone", "目标时区"),
      format(),
    ]),
    action("zones", "查找时区", [
      f("filter", "时区名称筛选", ""),
      f(
        "at",
        "偏移参考时刻",
        "",
        null,
        "可选 RFC 3339 日期时间；偏移不是全年固定值",
      ),
    ]),
  ],
  codec: [
    action(
      "base64-encode",
      "Base64 编码",
      [
        f("variant", "字母表", "standard", ["standard", "url-safe"]),
        f("padding", "填充", "padded", ["padded", "unpadded"]),
      ],
      "textfile",
    ),
    action(
      "base64-decode",
      "Base64 解码",
      [
        f("variant", "字母表", "standard", ["standard", "url-safe"]),
        f("padding", "填充", "auto", ["auto", "padded", "unpadded"]),
        f("ignore_whitespace", "忽略空白", "false", ["false", "true"]),
      ],
      "textfile",
      "解码结果按原始字节提供下载。",
    ),
    action(
      "url-parse",
      "解析 URL 参数",
      [
        f("mode", "解码规则", "form", ["form", "percent"]),
        f("query", "输入仅为 Query", "false", ["false", "true"]),
      ],
      "textfile",
    ),
    action(
      "html-encode",
      "HTML 实体编码",
      [
        f("standard", "标准", "html5", ["html5", "xml"]),
        f("style", "实体格式", "named", ["named", "decimal", "hex"]),
        f("context", "上下文", "text", ["text", "attribute"]),
        f("all", "编码所有字符", "false", ["false", "true"]),
      ],
      "textfile",
    ),
    action(
      "html-decode",
      "HTML 实体解码",
      [f("standard", "标准", "html5", ["html5", "xml"])],
      "textfile",
      "严格解码一次，不宽松或递归解码。",
    ),
  ],
  id: [
    action(
      "hash",
      "计算 Hash",
      [
        f("algorithm", "算法", "sha256", [
          "md5",
          "sha1",
          "sha224",
          "sha256",
          "sha384",
          "sha512",
        ]),
        f("format", "输出格式", "hex", ["hex", "base64"]),
      ],
      "textfile",
    ),
    action("uuid", "生成 UUID", [
      f("version", "版本", "v7", ["v4", "v7"]),
      f("format", "输出格式", "hyphenated", ["hyphenated", "simple", "urn"]),
      f("count", "数量（1–100）", "1"),
    ]),
  ],
  text: [
    action(
      "count",
      "字符统计",
      [f("bytes", "统计 UTF-8 字节", "false", ["false", "true"])],
      "textfile",
      "字符按 Unicode scalar value 计数。",
    ),
    action(
      "dedupe",
      "按行去重",
      [],
      "textfile",
      "保留首次出现的行及次序，空行也参与。",
    ),
    action(
      "case",
      "大小写转换",
      [
        f("mode", "转换方式", "lower", ["lower", "upper", "capitalize"]),
        f("words", "每个单词首字母", "false", ["false", "true"]),
      ],
      "textfile",
    ),
  ],
  key: [
    action(
      "generate",
      "生成密钥对",
      [
        f("algorithm", "算法", "ed25519", ["ed25519", "rsa"]),
        f("bits", "RSA 位数", "3072", ["2048", "3072", "4096"]),
        f("passphrase_mode", "私钥保护方式", "protected", [
          "protected",
          "unprotected",
        ]),
        f("passphrase", "私钥保护口令", ""),
      ],
      "none",
      "私钥 PKCS#8、公钥 SPKI；Ed25519 用于签名。私钥仅下载，不展示。",
    ),
    action(
      "public",
      "导出公钥",
      [f("passphrase", "私钥口令（如有）", "")],
      "key",
      "选择私钥文件。口令仅用于本次处理，完成请求后清空。",
    ),
    action(
      "check",
      "检查密钥",
      [f("passphrase", "私钥口令（如有）", "")],
      "keys",
      "首个文件为私钥；可选第二个公钥用于检查配对。",
    ),
  ],
  video: [
    action(
      "convert",
      "视频转换",
      [f("to", "目标格式", "mp4", ["mp4", "mkv", "webm", "mov"]), quality()],
      "media",
    ),
    action(
      "compress",
      "视频压缩",
      [f("quality", "编码质量", "small", ["high", "balanced", "small"])],
      "media",
    ),
    action(
      "trim",
      "截取片段",
      [f("start", "开始（秒）", "0"), f("duration", "时长（秒）", "5")],
      "media",
    ),
    action("resize", "调整画面尺寸", [f("size", "尺寸", "1280x720")], "media"),
    action("mute", "移除音轨", [], "media"),
    action("snapshot", "截取画面", [f("at", "截取时刻（秒）", "0")], "media"),
    action(
      "gif",
      "生成 GIF",
      [
        f("start", "开始（秒）", "0"),
        f("duration", "时长（最多60秒）", "5"),
        f("width", "宽度（2–4096）", "480"),
        f("fps", "帧率（1–50）", "12"),
      ],
      "media",
    ),
    action("audio-extract", "提取音轨", [audio()], "media"),
    action("audio-convert", "音频转换", [audio()], "media"),
  ],
  sound: [
    action("sfx", "合成音效", [
      f("preset", "音效", "success", [
        "notify",
        "success",
        "error",
        "coin",
        "jump",
        "laser",
        "explosion",
        "whoosh",
      ]),
      f("duration", "时长（0.05–10秒，可选）", ""),
      f("pitch", "音高偏移（-24–24）", "0"),
    ]),
    action("tone", "合成单音", [
      f("pitch", "音名或频率", "A4"),
      f("duration", "时长（0.01–120秒）", "1"),
      instrument(),
    ]),
    action(
      "melody",
      "编写旋律",
      [
        f("bpm", "BPM（20–400）", "120"),
        f("repeat", "重复（1–64）", "1"),
        f("format", "输出格式", "wav", ["wav", "mid"]),
      ],
      "score",
      "如 C4 D4 E4 G4:2（末音两拍），内部使用整数 tick；最多4096音符、120秒。MIDI 保存音符与节奏。",
    ),
    action("beat", "生成鼓点", [
      f("preset", "节奏", "basic", ["basic", "house", "hiphop"]),
      f("bpm", "BPM（20–400）", "120"),
      f("bars", "小节（1–64）", "4"),
    ]),
    action("presets", "查看音效预设"),
  ],
};
const labels = {
  protected: "使用口令保护（推荐）",
  unprotected: "明确不设置口令",
  jpeg: "JPEG（有损）",
  png: "PNG",
  webp: "WebP",
  tiff: "TIFF",
  avif: "AVIF",
  sine: "正弦波",
  soft: "柔和",
  bell: "铃声",
  pluck: "拨弦",
  chip: "芯片",
  max: "充分优化",
  fast: "快速优化",
  true: "是",
  false: "否",
  standard: "标准",
  "url-safe": "URL-safe",
  padded: "保留填充",
  unpadded: "不带填充",
  auto: "自动",
  form: "form（+ 为空格）",
  percent: "percent（+ 保留）",
  lower: "小写",
  upper: "大写",
  capitalize: "首字母大写",
  high: "高质量",
  balanced: "均衡",
  small: "较小体积",
};
const main = document.querySelector("#main");
const state = {
  page: "home",
  selected: {},
  drafts: {},
  files: {},
  jobs: {},
  busy: false,
  info: null,
};
const urls = new Set();
let toastTimer;
// 仅 token 进入 sessionStorage。输入、口令、上传文件和结果均只存在当前页面内存。
const bootstrap = new URLSearchParams(location.hash.slice(1));
let token =
  bootstrap.get("token") || sessionStorage.getItem("kito-token") || "";
if (bootstrap.has("token")) {
  sessionStorage.setItem("kito-token", token);
  history.replaceState(null, "", location.pathname + location.search + "#home");
}
async function api(path, options = {}) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 30000);
  try {
    const response = await fetch("/api/" + path, {
      ...options,
      headers: { "X-Kito-Token": token, ...options.headers },
      cache: "no-store",
      signal: controller.signal,
    });
    if (!response.ok) {
      let data;
      try {
        data = await response.json();
      } catch {}
      throw new Error(data?.error || `请求失败（${response.status}）`);
    }
    return options.binary
      ? response.blob()
      : response.status === 204
        ? null
        : response.json();
  } catch (error) {
    throw new Error(
      error.name === "AbortError"
        ? "请求超时，请检查本机服务后重试。"
        : error.message === "Failed to fetch"
          ? "无法连接本机服务，请确认 Kito 仍在运行后重试。"
          : error.message,
    );
  } finally {
    clearTimeout(timeout);
  }
}
function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}
function el(tag, cls, text) {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  if (text !== undefined) node.textContent = text;
  return node;
}
function toast(message) {
  const node = document.querySelector("#toast");
  node.textContent = message;
  node.classList.add("visible");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => node.classList.remove("visible"), 3500);
}
function button(text, fn, cls = "secondary") {
  const node = el("button", cls, text);
  node.type = "button";
  node.addEventListener("click", fn);
  return node;
}
function hydrateIcons(root = document) {
  root
    .querySelectorAll("[data-icon]")
    .forEach((node) => (node.innerHTML = icon(node.dataset.icon)));
}
const themeStyle = (tool) =>
  `--tone:${tool.tone};--tone-ink:${tool.ink};--dot:${tool.dot}`;
function renderNavigation() {
  document.querySelector("#navigation").innerHTML =
    `<a class="nav-link ${state.page === "home" ? "active" : ""}" ${state.page === "home" ? 'aria-current="page"' : ""} href="#home">${icon("grid")}工具小队${state.page === "home" ? '<span class="nav-dot"></span>' : ""}</a><div class="nav-heading">按功能找伙伴</div>${tools.map((tool) => `${tool.id === "text" ? '<div class="nav-separator"></div>' : ""}<a class="nav-link ${state.page === tool.id ? "active" : ""}" ${state.page === tool.id ? 'aria-current="page"' : ""} href="#${tool.id}">${icon(tool.id)}<span>${tool.name}</span>${tool.asset ? `<small class="nav-partner">${tool.partner.split(" ")[0]}</small>` : ""}</a>`).join("")}`;
}

function renderHome() {
  main.innerHTML = `<section class="welcome" aria-labelledby="welcome-heading">
    <div class="welcome-copy"><p class="welcome-intro">你好，今天想做点什么？</p><h1 id="welcome-heading">繁琐的小事，<br>交给顺手的伙伴。</h1><p class="description">处理图片、转换时间、创作声音……<br>各有所长的工具小队，陪你一起做好。</p><a href="#image" class="primary">从一张图片开始 ${icon("arrow")}</a></div>
    <div class="welcome-art"><span class="art-caption">很高兴，和你一起开工！</span><img src="assets/crew.svg" alt="露米、提可和可迪一起整理图片卡片" width="480" height="250"></div>
  </section>
  <div class="section-heading"><h2>找一位伙伴，开始吧<span>按你要做的事来选</span></h2><span>8 个工具领域</span></div>
  <section class="tool-grid" aria-label="所有工具">${tools.map((tool) => `<a href="#${tool.id}" class="tool-card" style="${themeStyle(tool)}" aria-label="${tool.name}，${tool.description}"><div class="card-glyph">${icon(tool.id)}</div>${tool.asset ? `<img class="card-mascot" src="assets/${tool.asset}.svg" alt="" width="77" height="77">` : '<span class="card-mascot text-mascot" aria-hidden="true">Aa</span>'}<h3>${tool.name}</h3><p>${tool.description}</p><div class="card-footer"><span><i></i>${tool.partner}${tool.title ? ` · ${tool.title}` : ""}</span>${icon("arrow")}</div></a>`).join("")}</section>
  <section class="care-strip"><div class="care-icon">${icon("shield")}</div><div><strong>好工具，也会照顾好你的原文件。</strong><p>延续 CLI 的习惯：默认另存新文件，清楚展示处理结果与验证信息。</p></div><button class="text-button" data-action="about">了解文件保护 ${icon("arrow-up-right")}</button></section>`;
}

function current() {
  return actions[state.page]?.find(
    (item) =>
      item.id === (state.selected[state.page] || actions[state.page][0].id),
  );
}
function keyFor(tool = state.page, item = current()) {
  return tool + ":" + item.id;
}
function saveDraft() {
  // 运行中控件 disabled，FormData 会跳过它们；保留提交前已保存的草稿。
  if (state.busy) return;
  const form = document.querySelector("#operation-form");
  if (!form) return;
  const draft = {};
  for (const [key, value] of new FormData(form))
    if (key !== "passphrase") draft[key] = value;
  state.drafts[form.dataset.key] = draft;
}
function revokeAll() {
  urls.forEach((url) => URL.revokeObjectURL(url));
  urls.clear();
}
function objectURL(blob) {
  const url = URL.createObjectURL(blob);
  urls.add(url);
  return url;
}
function renderTool() {
  revokeAll();
  const tool = tools.find((t) => t.id === state.page),
    item = current(),
    key = keyFor();
  main.innerHTML = `<header class="tool-heading">${tool.asset ? `<img src="assets/${tool.asset}.svg" alt="${tool.partner}" width="78" height="78">` : '<span class="text-mascot">Aa</span>'}<div><h1>${tool.name}</h1><p>${tool.partner}${tool.title ? " · " + tool.title : ""}。${tool.description}。</p></div><span class="partner-bubble">各有所长，一起把事情做好。</span></header><nav class="tabs" aria-label="处理能力"></nav><div id="backend-note" class="helper"></div><div class="image-layout"><section class="panel" id="input-panel"></section><aside><form class="panel options-panel" id="operation-form"><div class="panel-title"><h2>选择处理方式</h2>${icon(tool.id)}</div><div id="fields"></div><button class="primary" type="submit" id="run-button"></button><p class="truth-note">原文件不变，结果另行下载。</p></form><div class="lumi-note">${tool.asset ? `<img src="assets/${tool.asset}.svg" alt="" width="40" height="40">` : ""}<div><strong>伙伴的小叮咛</strong><p id="action-note"></p></div></div></aside></div><section class="panel result-panel" id="job-result" tabindex="-1" aria-live="polite" hidden></section>`;
  for (const tab of actions[state.page]) {
    const node = button(
      tab.label,
      () => {
        saveDraft();
        state.selected[state.page] = tab.id;
        renderTool();
      },
      "tab" + (item === tab ? " active" : ""),
    );
    node.setAttribute("aria-pressed", String(item === tab));
    node.disabled = state.busy;
    document.querySelector(".tabs").append(node);
  }
  document.querySelector("#action-note").textContent =
    item.note || tool.boundary;
  if (state.page === "video")
    document.querySelector("#action-note").textContent +=
      (item.note ? "。" : "") +
      "重新编码有损，不保证更小；字幕、附件与原有元数据不保留。音频统一为 48 kHz 立体声，可能量化或重采样。";
  const backend = state.info?.backends?.[state.page];
  const backendNote = document.querySelector("#backend-note");
  if (backend) {
    backendNote.textContent = backend.available
      ? "本机处理组件已就绪"
      : backend.message || "本机处理组件不可用，请安装相应依赖后重试。";
    // 完整组件版本保留在悬停说明中，常规操作无需阅读编译信息。
    if (backend.available) backendNote.title = backend.message || "";
  }
  if (state.page === "image" && backend?.available === false) {
    backendNote.textContent +=
      "。格式转换与调整尺寸需要 ImageMagick；PNG 压缩、元数据与图片信息仍可尝试。";
  }
  const form = document.querySelector("#operation-form");
  form.dataset.key = key;
  const draft = state.drafts[key] || {};
  for (const spec of item.fields) {
    const wrap = el("div", "form-field");
    const label = el("label", "field-label", spec.label);
    label.htmlFor = "param-" + spec.key;
    wrap.append(label);
    const control = el(spec.options ? "select" : "input", "select-field");
    control.id = "param-" + spec.key;
    control.name = spec.key;
    if (spec.options)
      for (const value of spec.options) {
        const option = el("option", "", labels[value] || value);
        option.value = value;
        control.append(option);
      }
    else control.type = spec.key === "passphrase" ? "password" : "text";
    control.value = draft[spec.key] ?? spec.value;
    control.autocomplete = "off";
    control.disabled = state.busy;
    if (["value", "datetime", "resize"].includes(spec.key))
      control.required = true;
    if (spec.key === "passphrase") {
      control.setAttribute("data-sensitive", "");
      control.maxLength = 1023;
    }
    wrap.append(control);
    if (spec.hint) wrap.append(el("p", "helper", spec.hint));
    document.querySelector("#fields").append(wrap);
  }
  // 不设口令必须明确选择；此 UI 字段不会进入后端参数。
  if (state.page === "key" && item.id === "generate") {
    const mode = document.querySelector("#param-passphrase_mode");
    const password = document.querySelector("#param-passphrase");
    const syncProtection = () => {
      password.required = mode.value === "protected";
      password.disabled = state.busy || mode.value === "unprotected";
      if (mode.value === "unprotected") password.value = "";
    };
    mode.addEventListener("change", syncProtection);
    syncProtection();
  }
  const panel = document.querySelector("#input-panel");
  const needsText = ["textfile", "score"].includes(item.input),
    needsFile =
      ["image", "media", "key", "keys"].includes(item.input) || needsText;
  const panelTitle = el("div", "panel-title");
  panel.append(panelTitle);
  panelTitle.append(
    el(
      "h2",
      "",
      needsText ? "输入内容" : needsFile ? "选择本机文件" : "准备好，就开始吧",
    ),
  );
  if (needsText) {
    const label = el(
      "label",
      "field-label",
      item.input === "score" ? "旋律乐谱" : "待处理文本",
    );
    label.htmlFor = "text-input";
    const textarea = el("textarea", "text-input");
    textarea.id = "text-input";
    textarea.name = "text";
    textarea.setAttribute("form", "operation-form");
    textarea.placeholder =
      item.input === "score"
        ? "例如：C4 D4 E4 G4:2"
        : "在这里输入原始文本；也可以选择文件";
    textarea.value = draft.text || "";
    textarea.disabled = state.busy;
    textarea.spellcheck = false;
    panel.append(label, textarea);
  }
  if (needsFile) {
    const drop = el("div", "dropzone");
    if (needsText) drop.classList.add("compact-drop");
    drop.innerHTML = `<div class="drop-illustration" aria-hidden="true"><div></div><div>${icon(tool.id)}</div></div><h3>${needsText ? "或选择文件" : item.input === "keys" ? "选择私钥与可选公钥" : "把文件放进来，伙伴来搭把手。"}</h3><p>拖入或选择文件；运行时上传至本机 Kito 服务<br>单文件上限 ${state.info?.limits?.upload_bytes ? (state.info.limits.upload_bytes / 1024 / 1024).toFixed(0) + " MiB" : "正在读取"}</p>`;
    if (item.input === "image")
      drop.append(
        el(
          "p",
          "helper",
          "支持 PNG、JPEG、WebP、GIF、BMP、TIFF、ICO、AVIF、HEIC 等像素图片容器。首版不接收 SVG、PDF 或脚本；实际格式由本机读取文件内容校验。",
        ),
      );
    const input = el("input");
    input.type = "file";
    input.hidden = true;
    input.multiple = item.input === "keys";
    input.id = "source-file";
    if (item.input === "image")
      input.accept =
        item.id === "compress"
          ? ".png,image/png"
          : ".png,.jpg,.jpeg,.webp,.gif,.bmp,.tif,.tiff,.ico,.avif,.heic,.heif";
    if (item.input === "media") input.accept = "video/*,audio/*";
    const choose = button(
      "选择" + (item.input === "keys" ? "文件" : "本机文件"),
      () => input.click(),
      "primary",
    );
    choose.disabled = state.busy;
    drop.append(choose, input);
    panel.append(drop);
    const select = (files) => {
      if (state.busy) return;
      const max = item.input === "keys" ? 2 : 1;
      if (files.length > max) {
        toast(`本次最多选择 ${max} 个文件。`);
        return;
      }
      if (
        state.info?.limits?.upload_bytes &&
        [...files].some((file) => file.size > state.info.limits.upload_bytes)
      ) {
        toast("文件超过本机服务的单文件上限，请选择较小文件。");
        return;
      }
      saveDraft();
      state.files[key] = [...files];
      if (needsText) state.drafts[key].text = "";
      renderTool();
    };
    input.addEventListener("change", () => select(input.files));
    drop.addEventListener("dragover", (event) => {
      event.preventDefault();
      drop.classList.add("dragover");
    });
    drop.addEventListener("dragleave", () => drop.classList.remove("dragover"));
    drop.addEventListener("drop", (event) => {
      event.preventDefault();
      select(event.dataTransfer.files);
    });
    const files = state.files[key] || [];
    for (const file of files) {
      const row = el("div", "file-row");
      const meta = el("div", "file-meta");
      meta.append(
        el("strong", "", file.name),
        el("span", "", formatBytes(file.size)),
      );
      row.append(
        meta,
        button(
          "移除",
          () => {
            saveDraft();
            state.files[key] = files.filter((candidate) => candidate !== file);
            renderTool();
          },
          "text-button",
        ),
      );
      panel.append(row);
    }
    if (files[0]?.type.startsWith("image/") && item.input === "image") {
      const preview = el("img", "preview-image");
      preview.alt = "所选图片的本地预览";
      preview.src = objectURL(files[0]);
      panel.append(preview);
    }
    if (needsText) {
      const text = document.querySelector("#text-input");
      text.disabled = state.busy || files.length > 0;
      if (files.length)
        panel.append(
          el(
            "p",
            "helper",
            "已选择文件，按文件原始字节处理。移除文件后可输入文本。",
          ),
        );
    }
    panel.append(
      el("p", "file-note", "只读取所选文件；不会覆盖或修改你的原文件。"),
    );
  } else {
    panel.innerHTML += `<div class="ready-art">${tool.asset ? `<img src="assets/${tool.asset}.svg" alt="" width="150" height="150">` : icon(tool.id)}</div>`;
    panel.append(
      el(
        "p",
        "helper",
        "选择右侧参数，点击按钮运行。所有结果来自当前本机 Kito。",
      ),
    );
  }
  const run = document.querySelector("#run-button");
  run.textContent = state.busy ? "正在处理…" : item.label;
  const requiresBackend =
    state.page !== "image" || ["convert", "resize"].includes(item.id);
  run.disabled =
    state.busy || (requiresBackend && backend?.available === false);
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    runJob(item, key);
  });
  renderResult(key);
}
async function runJob(item, key) {
  if (state.busy) return;
  const form = document.querySelector("#operation-form");
  if (!form.reportValidity()) return;
  const params = Object.fromEntries(new FormData(form));
  for (const name of Object.keys(params))
    if (params[name] === "" && name !== "text") delete params[name];
  delete params.passphrase_mode;
  const files = state.files[key] || [];
  if (["image", "media", "key", "keys"].includes(item.input) && !files.length) {
    toast("请先选择本机文件。");
    return;
  }
  if (files.length) delete params.text;
  if (state.page === "key" && params.algorithm === "ed25519")
    delete params.bits;
  const tool = state.page;
  saveDraft();
  state.busy = true;
  state.jobs[key] = { state: "uploading" };
  renderTool();
  try {
    const ids = [];
    for (const file of files) {
      const upload = await api(
        "uploads?name=" + encodeURIComponent(file.name),
        { method: "POST", body: file },
      );
      ids.push(upload.id);
    }
    const started = await api("jobs", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ tool, action: item.id, params, files: ids }),
    });
    // 请求已发送后立即抹去口令，不把它纳入重试缓存或可复制结果。
    delete params.passphrase;
    document
      .querySelectorAll("[data-sensitive]")
      .forEach((node) => (node.value = ""));
    state.jobs[key] = { ...started };
    renderResult(key);
    await poll(key, started.id);
  } catch (error) {
    state.jobs[key] = { state: "failed", error: error.message };
  } finally {
    delete params.passphrase;
    document
      .querySelectorAll("[data-sensitive]")
      .forEach((node) => (node.value = ""));
    state.busy = false;
    // 用户可在任务运行时浏览其他领域，结束后也要恢复当前页面控件。
    if (current()) renderTool();
    if (state.page === tool && keyFor() === key) focusResult();
  }
}
function focusResult() {
  const result = document.querySelector("#job-result");
  if (result && !result.hidden) result.focus({ preventScroll: true });
}
async function poll(key, id) {
  // 串行轮询、最多三次连续网络重试，避免断连后无限请求或重复创建任务。
  let failures = 0;
  const deadline = Date.now() + 210000;
  while (Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 800));
    try {
      const job = await api("jobs/" + encodeURIComponent(id));
      failures = 0;
      state.jobs[key] = job;
      renderResult(key);
      if (job.state !== "running") return;
    } catch (error) {
      failures++;
      if (failures >= 3) {
        state.jobs[key] = { id, state: "disconnected", error: error.message };
        return;
      }
      await new Promise((resolve) => setTimeout(resolve, failures * 700));
    }
  }
  state.jobs[key] = {
    id,
    state: "disconnected",
    error: "等待结果已超过时限。可以重新查询已有任务。",
  };
}
// 摘要字段对应 CLI 的真实 Report 类型；未知结构保留折叠报告，不猜测含义。
function appendCopyable(container, content, label = "复制结果") {
  container.append(
    el("pre", "result-text", content),
    button(
      label,
      async () => {
        try {
          await navigator.clipboard.writeText(content);
          toast("结果已复制。");
        } catch {
          toast("无法访问剪贴板，可选中结果手动复制。");
        }
      },
      "secondary",
    ),
  );
}
function renderReportSummary(panel, report) {
  if (state.page === "time" && typeof report.datetime === "string") {
    panel.append(el("p", "summary-label", "日期时间"));
    appendCopyable(panel, report.datetime);
    if (typeof report.timezone === "string")
      panel.append(el("p", "helper", "时区：" + report.timezone));
    if (typeof report.value === "string" && report.value !== report.datetime) {
      panel.append(el("p", "summary-label", "时间戳"));
      appendCopyable(panel, report.value);
    }
    if (report.precision_lost === true)
      panel.append(el("p", "helper", "降低时间戳精度时已向负无穷取整。"));
  } else if (state.page === "id" && typeof report.digest === "string") {
    panel.append(el("p", "summary-label", "内容指纹"));
    appendCopyable(panel, report.digest);
  } else if (
    state.page === "id" &&
    Array.isArray(report.values) &&
    report.values.every((value) => typeof value === "string")
  ) {
    panel.append(
      el("p", "summary-label", `已生成 ${report.values.length} 个 UUID`),
    );
    appendCopyable(panel, report.values.join("\n"));
  } else if (
    state.page === "sound" &&
    typeof report.duration_seconds === "number"
  ) {
    panel.append(
      el(
        "p",
        "summary-label",
        `声音已生成 · ${report.duration_seconds.toFixed(2)} 秒`,
      ),
    );
    if (typeof report.format === "string")
      panel.append(
        el(
          "p",
          "helper",
          "输出格式：" + report.format.toUpperCase() + "，请下载或预览结果。",
        ),
      );
  } else if (state.page === "image") {
    const entries = Array.isArray(report.files) ? report.files : [report];
    for (const entry of entries) {
      if (!entry || typeof entry !== "object") continue;
      if (
        typeof entry.input_bytes === "number" &&
        typeof entry.output_bytes === "number"
      )
        panel.append(
          el(
            "p",
            "summary-label",
            `文件大小：${formatBytes(entry.input_bytes)} → ${formatBytes(entry.output_bytes)}`,
          ),
        );
      const verification = entry.conversion || entry;
      if (verification.verified_lossless === true)
        panel.append(el("p", "inline-check", "已通过原始像素无损验证。"));
      else if (verification.encoding_verified_lossless === true)
        panel.append(
          el(
            "p",
            "inline-check",
            "编码无损验证通过；不代表结果与原图逐像素一致。",
          ),
        );
      else if (verification.verified_lossless === false)
        panel.append(
          el("p", "helper", "此结果未标记为原图像素无损，请查看详细报告。"),
        );
    }
  }
}
function renderResult(key) {
  if (!current() || keyFor() !== key) return;
  const panel = document.querySelector("#job-result");
  if (!panel) return;
  panel.replaceChildren();
  const job = state.jobs[key];
  panel.hidden = !job;
  if (!job) return;
  const titles = {
    uploading: "正在上传至本机…",
    running: "伙伴正在处理…",
    completed: "处理完成",
    failed: "本次处理未完成",
    disconnected: "连接中断，结果仍可重试查询",
  };
  panel.classList.toggle(
    "result-failure",
    ["failed", "disconnected"].includes(job.state),
  );
  panel.append(el("h2", "result-title", titles[job.state] || job.state));
  if (job.error)
    panel.append(
      el(
        "p",
        "error",
        typeof job.error === "string" ? job.error : JSON.stringify(job.error),
      ),
    );
  if (job.state === "disconnected")
    panel.append(
      button(
        "重新查询结果",
        async () => {
          if (state.busy) return;
          state.busy = true;
          try {
            await poll(key, job.id);
          } finally {
            state.busy = false;
            if (current()) renderTool();
            if (current() && keyFor() === key) focusResult();
          }
        },
        "secondary",
      ),
    );
  const report = job.report;
  const structured = report !== null && typeof report === "object";
  if (structured) {
    renderReportSummary(panel, report);
    const details = el("details");
    details.append(el("summary", "", "详细报告"));
    appendCopyable(details, JSON.stringify(report, null, 2), "复制完整报告");
    panel.append(details);
  } else if (job.stdout && state.page !== "key") {
    appendCopyable(panel, job.stdout, "复制结果");
  }
  for (const file of job.files || []) {
    const row = el("div", "file-row");
    const meta = el("div", "file-meta");
    meta.append(
      el("strong", "", file.name),
      el("span", "", formatBytes(file.size)),
    );
    const download = button(
      "下载",
      async () => {
        download.disabled = true;
        try {
          const blob = await api("files/" + encodeURIComponent(file.id), {
            binary: true,
          });
          const link = el("a");
          link.href = objectURL(blob);
          link.download = file.name;
          document.body.append(link);
          link.click();
          link.remove();
        } catch (error) {
          toast(error.message);
        } finally {
          download.disabled = false;
        }
      },
      "secondary",
    );
    row.append(meta, download);
    panel.append(row);
    if (
      state.page !== "key" &&
      /^text\/plain;\s*charset=utf-8$/i.test(file.mime || "")
    ) {
      const textPreview = button(
        "查看文本",
        async () => {
          textPreview.disabled = true;
          try {
            const blob = await api("files/" + encodeURIComponent(file.id), {
              binary: true,
            });
            const content = new TextDecoder("utf-8", { fatal: true }).decode(
              await blob.arrayBuffer(),
            );
            const pre = el("pre", "result-text", content);
            row.after(pre);
            row.append(
              button(
                "复制文本",
                async () => {
                  try {
                    await navigator.clipboard.writeText(content);
                    toast("文本已复制。");
                  } catch {
                    toast("无法访问剪贴板，可选中文本手动复制。");
                  }
                },
                "text-button",
              ),
            );
            textPreview.remove();
          } catch (error) {
            toast(
              error instanceof TypeError
                ? "结果不是有效的 UTF-8 文本，请下载原始字节。"
                : error.message,
            );
            textPreview.disabled = false;
          }
        },
        "text-button",
      );
      row.append(textPreview);
    }
    if (
      state.page !== "key" &&
      /^(image\/(png|jpeg|webp|gif)|audio\/)/.test(file.mime || "")
    ) {
      const preview = button(
        "预览",
        async () => {
          preview.disabled = true;
          try {
            const blob = await api("files/" + encodeURIComponent(file.id), {
              binary: true,
            });
            const media = el(
              file.mime.startsWith("image/") ? "img" : "audio",
              file.mime.startsWith("image/")
                ? "preview-image"
                : "audio-preview",
            );
            media.src = objectURL(blob);
            if (media.tagName === "AUDIO") media.controls = true;
            else media.alt = "处理结果预览";
            row.after(media);
            preview.remove();
          } catch (error) {
            toast(error.message);
            preview.disabled = false;
          }
        },
        "text-button",
      );
      row.append(preview);
    }
  }
}
function setMenu(open, moveFocus = false) {
  const mobile = matchMedia("(max-width: 720px)").matches;
  const expanded = mobile && open;
  document.body.classList.toggle("menu-open", expanded);
  document.querySelector(".nav-backdrop").hidden = !expanded;
  document
    .querySelector(".menu-toggle")
    .setAttribute("aria-expanded", String(expanded));
  document.querySelector(".sidebar").inert = mobile && !expanded;
  document.querySelector(".app-body").inert = expanded;
  if (moveFocus)
    (expanded
      ? document.querySelector(".sidebar .brand")
      : document.querySelector(".menu-toggle")
    ).focus();
}
function route(focus = false, skipSave = false) {
  if (!skipSave) saveDraft();
  const id = location.hash.slice(1).split("/")[0];
  state.page = tools.some((tool) => tool.id === id) ? id : "home";
  document.querySelector("#breadcrumb").textContent =
    state.page === "home"
      ? "工具小队"
      : "工具小队 / " + tools.find((tool) => tool.id === state.page).name;
  renderNavigation();
  if (state.page === "home") renderHome();
  else renderTool();
  document.title =
    (state.page === "home"
      ? "工具小队"
      : tools.find((tool) => tool.id === state.page).name) + " · Kito 奇托";
  setMenu(false);
  document.querySelectorAll("dialog[open]").forEach((dialog) => dialog.close());
  if (focus) {
    main.focus({ preventScroll: true });
    window.scrollTo(0, 0);
  }
}
function showInfo() {
  document.querySelector("#info-title").textContent =
    "各有所长，一起把事情做好";
  const content = document.querySelector("#info-content");
  content.replaceChildren(
    el(
      "p",
      "",
      "Kito 奇托把图片、时间、编解码、标识、密钥、视频、声音与文本工具带到浏览器。处理由当前电脑上的 Kito 完成。",
    ),
    el("h3", "", "原文件安好，结果另存"),
    el(
      "p",
      "",
      "所选文件只上传到本机服务的临时会话。结果通过下载保存，不覆盖你的原文件。关闭 Kito 后会清理临时会话；也可以在空闲时立即清空。",
    ),
    el("h3", "", "能力来自本机"),
    el(
      "p",
      "",
      state.info
        ? "Kito " +
            state.info.version +
            " · 图片、视频和密钥的可用能力取决于本机处理后端。"
        : "正在读取本机版本与能力。",
    ),
    el(
      "p",
      "",
      "页面只保存当前连接 token，不保存输入或口令。私钥仅允许下载，请妥善保管。",
    ),
  );
  document.querySelector("#info-dialog").showModal();
}
function renderSearch(query = "") {
  const value = query.trim().toLowerCase();
  const results = document.querySelector("#search-results");
  results.replaceChildren();
  for (const tool of tools.filter((tool) =>
    `${tool.name} ${tool.partner} ${tool.description} ${tool.id} ${actions[tool.id].map((item) => item.label).join(" ")}`
      .toLowerCase()
      .includes(value),
  )) {
    const link = el("a", "search-result");
    link.href = "#" + tool.id;
    if (tool.asset) {
      const image = el("img");
      image.src = "assets/" + tool.asset + ".svg";
      image.alt = "";
      link.append(image);
    }
    const text = el("div");
    text.append(el("strong", "", tool.name), el("small", "", tool.description));
    link.append(text);
    link.addEventListener("click", () =>
      document.querySelector("#search-dialog").close(),
    );
    results.append(link);
  }
  if (!results.children.length)
    results.append(
      el("p", "helper", "还没找到这个工具，试试「图片」「时间」或伙伴名字。"),
    );
}
async function loadInfo() {
  const badge = document.querySelector(".preview-badge");
  try {
    state.info = await api("info");
    badge.textContent = "本机已连接";
    document.querySelector(".workspace-note div span").textContent =
      "Kito " + state.info.version;
    if (state.page !== "home" && !state.busy) {
      saveDraft();
      renderTool();
    }
  } catch (error) {
    badge.textContent = "重试连接";
    toast(error.message);
  }
}
document.addEventListener("click", async (event) => {
  if (event.target.closest(".skip-link")) {
    event.preventDefault();
    main.focus();
    return;
  }
  const target = event.target.closest("[data-action]");
  if (!target) return;
  switch (target.dataset.action) {
    case "menu":
      setMenu(!document.body.classList.contains("menu-open"), true);
      break;
    case "search":
      renderSearch();
      document.querySelector("#tool-search").value = "";
      document.querySelector("#search-dialog").showModal();
      document.querySelector("#tool-search").focus();
      break;
    case "close":
      target.closest("dialog").close();
      break;
    case "about":
    case "principles":
      showInfo();
      break;
    case "connection":
      await loadInfo();
      break;
    case "clear":
      if (state.busy) {
        toast("伙伴正在处理，请等任务结束后再清空。");
        return;
      }
      target.disabled = true;
      try {
        await api("session", { method: "DELETE" });
        revokeAll();
        state.drafts = {};
        state.files = {};
        state.jobs = {};
        route(false, true);
        toast("本次上传和结果已清空。");
      } catch (error) {
        toast(error.message);
      } finally {
        target.disabled = false;
      }
      break;
  }
});
document.querySelector(".preview-badge").dataset.action = "connection";
const clear = button("清空本次文件", () => {}, "sidebar-help");
clear.dataset.action = "clear";
document.querySelector(".sidebar-bottom").prepend(clear);
document
  .querySelector("#tool-search")
  .addEventListener("input", (event) => renderSearch(event.target.value));
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && document.querySelector("dialog[open]")) {
    event.preventDefault();
    document.querySelector("dialog[open]").close();
    return;
  }
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    document.querySelector('[data-action="search"]').click();
  }
  if (document.body.classList.contains("menu-open")) {
    if (event.key === "Escape") {
      setMenu(false, true);
      return;
    }
    if (event.key === "Tab") {
      const items = [
        ...document.querySelectorAll(".sidebar a,.sidebar button"),
      ];
      if (event.shiftKey && document.activeElement === items[0]) {
        event.preventDefault();
        items.at(-1).focus();
      } else if (!event.shiftKey && document.activeElement === items.at(-1)) {
        event.preventDefault();
        items[0].focus();
      }
    }
  }
});
window.addEventListener("hashchange", () => route(true));
window.addEventListener("beforeunload", revokeAll);
matchMedia("(max-width: 720px)").addEventListener("change", () =>
  setMenu(false),
);
hydrateIcons();
route();
loadInfo();
