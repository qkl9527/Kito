/* 这是可丢弃的交互设计稿，不调用 CLI、不上传文件、不生成压缩结果。
 * 文件只通过 Object URL 在当前页面预览；所有结果数字均属于固定示例。
 * 不把本原型的 UI 状态直接当成未来 Web 后端协议。
 */
const icons = {
  grid: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>',
  image: '<rect x="3" y="3" width="18" height="18" rx="3"/><circle cx="8" cy="8" r="1.5"/><path d="m3 16 5-5 5 6 3-3 5 5"/>',
  time: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l4 2"/>',
  codec: '<path d="m8 6-6 6 6 6m8-12 6 6-6 6m-3-14-2 16"/>',
  id: '<path d="M9 3 7 21M17 3l-2 18M3 9h18M2 15h18"/>',
  key: '<circle cx="8" cy="8" r="5"/><path d="m12 12 9 9m-5-5 3-3m-1 5 3-3"/>',
  video: '<rect x="3" y="4" width="18" height="16" rx="3"/><path d="m10 8 6 4-6 4z"/>',
  sound: '<path d="M3 10v4m4-8v12m5-16v20m5-16v12m4-8v4"/>',
  text: '<path d="M3 5h12M9 5v15m6-8h7m-4-3v11"/>',
  search: '<circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/>',
  arrow: '<path d="M4 12h16m-6-6 6 6-6 6"/>',
  'arrow-up-right': '<path d="M6 18 18 6M7 6h11v11"/>',
  close: '<path d="m6 6 12 12M6 18 18 6"/>',
  shield: '<path d="m12 3 8 3v6c0 5-8 9-8 9s-8-4-8-9V6z"/><path d="m8 12 3 3 5-6"/>',
  check: '<path d="m5 12 4 4L19 6"/>',
  monitor: '<rect x="3" y="3" width="18" height="13" rx="2"/><path d="M8 21h8m-4-5v5"/>',
  book: '<path d="M12 5c-3-2-7-2-10-1v15c3-1 7-1 10 1 3-2 7-2 10-1V4c-3-1-7-1-10 1zm0 0v15"/>',
  upload: '<path d="M12 16V3m-5 5 5-5 5 5M4 15v5h16v-5"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6m0-10v1"/>',
  terminal: '<path d="m4 6 6 6-6 6m9 0h7"/>',
  menu: '<path d="M4 6h16M4 12h16M4 18h16"/>',
  replay: '<path d="M3 10a9 9 0 1 1 1 7M3 4v6h6"/>',
};
const icon = (name) => `<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${icons[name] || icons.grid}</svg>`;
const escapeHtml = (value) => String(value).replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char]));
const tools = [
  { id: 'image', name: '图片处理', partner: 'Lumi 露米', title: '光影师', asset: 'lumi', description: '压缩 PNG、转换格式、调整尺寸', tone: '#fff4d6', ink: '#9b7823', dot: '#d6ad45', abilities: [] },
  { id: 'time', name: '时间转换', partner: 'Tiko 提可', title: '时刻向导', asset: 'tiko', description: '时间戳转换与全球时区换算', tone: '#e6f2ec', ink: '#50836b', dot: '#87b49b', abilities: [['时间戳互转', '在 Unix 时间戳与日期时间之间转换，选择秒、毫秒、微秒或纳秒。'], ['跨时区转换', '选定来源和目标时区，把同一个时刻换算清楚。'], ['全球时区查询', '按地区、名称或偏移查找时区，偏移与参考时刻一起展示。'], ['默认时区', '保存常用时区，减少重复选择。']], boundary: '夏令时带来的歧义或不存在的时间会明确提示；降低时间戳精度时说明取整。' },
  { id: 'codec', name: '编解码', partner: 'Codi 可迪', title: '译码师', asset: 'codi', description: 'Base64、URL 参数与 HTML 实体', tone: '#efebf9', ink: '#80709f', dot: '#af9bce', abilities: [['Base64 编解码', '支持标准与 URL-safe 字母表，把文本或文件转换成需要的表示。'], ['URL 参数解析', '清楚列出每个参数，保留重复参数、空值与原始次序。'], ['HTML / XML 实体', '编码或解码 HTML5 与 XML 1.0 实体，一次只处理一层。']], boundary: '编码不等于加密；HTML 实体转换也不用于清洗不安全的 HTML。' },
  { id: 'id', name: '指纹与标识', partner: 'Sumi 苏米', title: '印记师', asset: 'sumi', description: '计算内容 Hash，生成 UUID', tone: '#f9eae2', ink: '#a1735a', dot: '#d49b7d', abilities: [['计算 Hash', '按文本或文件的真实字节计算摘要，支持六种算法。'], ['生成 UUID', '选择 UUID v4 或 v7，为对象生成标识。']], boundary: 'Hash 是内容指纹，UUID 是对象标识；UUID 不能替代密钥或授权凭证。' },
  { id: 'key', name: '密钥工具', partner: 'Kavi 卡维', title: '钥匙匠', asset: 'kavi', description: '生成密钥对，导出与检查公钥', tone: '#e9f0fa', ink: '#5e7d9e', dot: '#8daecb', abilities: [['生成密钥对', '使用 OpenSSL 3.x 生成 RSA 或 Ed25519 密钥，保存到新的独立目录。'], ['导出公钥', '从私钥中导出 SPKI 格式的公钥。'], ['检查配对', '确认私钥与公钥是否匹配，并检查后端状态。']], boundary: 'Ed25519 用于签名。私钥和口令不进入报告；正式 Web 版需要独立设计口令输入与敏感信息生命周期。' },
  { id: 'video', name: '视频与音频', partner: 'Vido 维多', title: '剪影师', asset: 'vido', description: '剪辑与转换视频，处理现有音频', tone: '#f8e8eb', ink: '#a26d78', dot: '#d699a2', abilities: [['转换与压缩', '将本地视频转换为下一步使用的格式，也可以调整尺寸与播放速度。'], ['剪辑与画面', '裁剪片段、裁切画面、旋转、拼接，或去除音轨。'], ['现有音频处理', '提取音轨，转换音乐格式，剪辑、混音、淡化或调整响度。'], ['截图与 GIF', '从本地视频截取 PNG，或生成短小的 GIF。']], boundary: '视频重新编码有损，无法保证文件更小。字幕、附件和原有元数据不保留；实际执行依赖本机 FFmpeg / ffprobe。' },
  { id: 'sound', name: '声音创作', partner: 'Echo 艾可', title: '音效师', asset: 'echo', description: '从零制作音效、旋律与鼓点', tone: '#edecf9', ink: '#76759f', dot: '#a7a1c7', abilities: [['一键音效', '选择场景，合成提示音与游戏音效。'], ['单音与旋律', '用音名或频率生成单音，用和弦与休止编写旋律。默认输出 WAV，旋律可另存 MIDI。'], ['鼓点创作', '选择节奏预设，或用 16 格节奏编排自己的鼓点。']], boundary: '使用合成音色，不是真实乐器采样；MIDI 保存音符与节奏，音色由播放软件决定。现有音乐编辑请交给 Vido。' },
  { id: 'text', name: '文本工具', partner: '日常小工具', title: '', asset: null, description: '字符统计、按行去重、大小写转换', tone: '#edf0f3', ink: '#6b7885', dot: '#a7b0b8', abilities: [['字符统计', '统计 Unicode 字符数量或 UTF-8 字节数。'], ['按行去重', '保留第一次出现的行及其顺序，空行也参与去重。'], ['大小写转换', '转换大写、小写，或调整首字符与单词首字母。']], boundary: '输入文件必须是有效的 UTF-8 文本。字符计数按 Unicode scalar value，不等同于视觉上的字素簇。' },
];

const state = { page: 'home', imageTab: 'compress', sample: false, file: null, objectUrl: null, result: false, effort: 'max', error: '', resultKind: 'success' };
let toastTimer;
let fileReadVersion = 0;
const main = document.querySelector('#main');
const themeStyle = (tool) => `--tone:${tool.tone};--tone-ink:${tool.ink};--dot:${tool.dot}`;

function hydrateIcons(root = document) {
  root.querySelectorAll('[data-icon]').forEach((element) => { element.innerHTML = icon(element.dataset.icon); });
}

function renderNavigation() {
  document.querySelector('#navigation').innerHTML = `<a class="nav-link ${state.page === 'home' ? 'active' : ''}" ${state.page === 'home' ? 'aria-current="page"' : ''} href="#home">${icon('grid')}工具小队${state.page === 'home' ? '<span class="nav-dot"></span>' : ''}</a><div class="nav-heading">按功能找伙伴</div>${tools.map((tool) => `${tool.id === 'text' ? '<div class="nav-separator"></div>' : ''}<a class="nav-link ${state.page === tool.id ? 'active' : ''}" ${state.page === tool.id ? 'aria-current="page"' : ''} href="#${tool.id}">${icon(tool.id)}<span>${tool.name}</span>${tool.asset ? `<small class="nav-partner">${tool.partner.split(' ')[0]}</small>` : ''}</a>`).join('')}`;
}

function renderHome() {
  main.innerHTML = `<section class="welcome" aria-labelledby="welcome-heading">
    <div class="welcome-copy"><p class="welcome-intro">你好，今天想做点什么？</p><h1 id="welcome-heading">繁琐的小事，<br>交给顺手的伙伴。</h1><p class="description">处理图片、转换时间、创作声音……<br>各有所长的工具小队，陪你一起做好。</p><a href="#image" class="primary">从一张图片开始 ${icon('arrow')}</a></div>
    <div class="welcome-art"><span class="art-caption">很高兴，和你一起开工！</span><img src="assets/crew.svg" alt="露米、提可和可迪一起整理图片卡片" width="480" height="250"></div>
  </section>
  <div class="section-heading"><h2>找一位伙伴，开始吧<span>按你要做的事来选</span></h2><span>8 个工具领域</span></div>
  <section class="tool-grid" aria-label="所有工具">${tools.map((tool) => `<a href="#${tool.id}" class="tool-card" style="${themeStyle(tool)}" aria-label="${tool.name}，${tool.description}"><div class="card-glyph">${icon(tool.id)}</div>${tool.asset ? `<img class="card-mascot" src="assets/${tool.asset}.svg" alt="" width="77" height="77">` : '<span class="card-mascot text-mascot" aria-hidden="true">Aa</span>'}<h3>${tool.name}</h3><p>${tool.description}</p><div class="card-footer"><span><i></i>${tool.partner}${tool.title ? ` · ${tool.title}` : ''}</span>${icon('arrow')}</div></a>`).join('')}</section>
  <section class="care-strip"><div class="care-icon">${icon('shield')}</div><div><strong>好工具，也会照顾好你的原文件。</strong><p>延续 CLI 的习惯：默认另存新文件，清楚展示处理结果与验证信息。</p></div><button class="text-button" data-action="principles">了解设计原则 ${icon('arrow-up-right')}</button></section>`;
}

function filePanel() {
  const selected = state.sample || state.file;
  return `<section class="panel"><div class="panel-title"><h2>选择一张图片</h2><span>${selected ? (state.sample ? '示例素材' : '仅浏览器本地预览') : '单张图片，轻松开始'}</span></div>
    ${state.error ? `<p class="error" role="alert">${escapeHtml(state.error)}</p>` : ''}
    ${selected ? `<img class="preview-image" src="${state.sample ? 'assets/landscape.svg' : state.objectUrl}" alt="${state.sample ? '山间来信：层叠山丘、黄色小屋与蜿蜒步道的插画示例' : '所选图片的本地预览'}"><div class="file-row"><div class="file-type">PNG</div><div class="file-meta"><strong>${state.sample ? '山间来信.png' : escapeHtml(state.file.name)}</strong><span>${state.sample ? '2400 × 1800 px · 2.40 MB（示例）' : `${escapeHtml(state.file.size)} · 尚未进行后端校验`}</span></div>${state.sample ? '<span class="sample-tag">示例</span>' : ''}<button class="icon-button" data-action="remove-file" aria-label="移除当前图片">${icon('close')}</button></div>` : `<div class="dropzone" id="dropzone"><div class="drop-illustration" aria-hidden="true"><div></div><div>${icon('image')}</div></div><h3>把图片放进来，露米来搭把手。</h3><p>${state.imageTab === 'compress' ? '选择或拖入一张静态 PNG 图片' : '此稿用 PNG 预览输入与参数布局'}<br>文件只在当前浏览器预览，不会上传</p><div class="drop-actions"><button class="primary" data-action="choose-file">${icon('upload')}选择 PNG 图片</button><button class="text-button" data-action="sample">先用示例试试 ${icon('arrow')}</button></div></div>`}
    <input id="image-file" type="file" accept="image/png,.png" hidden>
    <div class="file-note">${icon('shield')}<span>原文件不变。设计稿只演示操作流程，不执行图片处理。</span></div>
    <div class="steps" aria-label="处理步骤"><span class="${!selected ? 'current' : ''}"><i>1</i>选好图片</span><span class="${selected && !state.result ? 'current' : ''}"><i>2</i>选择处理方式</span><span class="${state.result ? 'current' : ''}"><i>3</i>核对结果</span></div>
  </section>`;
}

function optionsPanel() {
  const selected = state.sample || state.file;
  const sourceName = state.sample ? '山间来信' : state.file ? state.file.name.replace(/\.png$/i, '') : '图片名称';
  const tab = state.imageTab;
  return `<aside><section class="panel options-panel"><div class="panel-title"><h2>${tab === 'compress' ? '让图片轻一点' : tab === 'convert' ? '换成需要的格式' : '调到合适的尺寸'}</h2>${icon(tab === 'compress' ? 'image' : 'grid')}</div>
    ${tab === 'compress' ? `<span class="field-label">优化方式</span><label class="choice"><input type="radio" name="effort" value="max" ${state.effort === 'max' ? 'checked' : ''}><span><strong>充分优化</strong><small>多种策略择优，愿意多等一会儿</small></span><span class="recommended">默认</span></label><label class="choice"><input type="radio" name="effort" value="fast" ${state.effort === 'fast' ? 'checked' : ''}><span><strong>快速优化</strong><small>减少搜索，优先处理速度</small></span></label><p class="helper">两种方式都保持原始像素，不降低画质。<br>压缩效果取决于图片本身。</p>` : tab === 'convert' ? '<label class="field-label" for="format-choice">目标格式</label><select class="select-field" id="format-choice"><option>WebP（无损编码）</option><option>PNG</option><option>TIFF</option></select><p class="helper">正式模式需按本机 ImageMagick 编解码能力提供选项。本稿仅展示参数布局。</p>' : '<label class="field-label" for="resize-choice">缩放比例</label><select class="select-field" id="resize-choice"><option>50% · 宽高缩小一半</option><option>75% · 宽高缩小四分之一</option><option>200% · 宽高放大一倍</option></select><p class="helper">保持宽高比。尺寸变化会重采样，编码验证仅针对缩放后的参考图。</p>'}
    <div class="separator"></div><span class="field-label">保存为新文件</span><div class="output-preview" id="output-name">${escapeHtml(sourceName)}.${tab === 'compress' ? 'optimized.png' : tab === 'convert' ? 'webp' : 'resized.png'}</div><div class="inline-check">${icon('check')}保留原文件，不覆盖已有文件</div><p class="helper">正式模式会在处理前确认保存位置。</p>
    <details><summary>更多说明</summary><p>${tab === 'compress' ? '默认保持尺寸与元数据。不提供有损质量滑杆；需要改变尺寸时，请使用「调整尺寸」。' : '格式与尺寸改变可能影响精度、透明度或元数据，结果需要后端验证后才能发布。'}</p><p>若未找到更小且合格的压缩结果，保留原内容。</p></details>
    <button class="primary" data-action="demo" ${!selected ? 'disabled' : ''}>${icon('image')}${tab === 'compress' ? '查看压缩结果示例' : '查看流程说明'}</button><p class="truth-note">${state.file ? '你的图片仅供预览；演示结果使用固定示例。' : '交互演示，不会创建或修改任何文件。'}</p>
  </section><div class="lumi-note"><img src="assets/lumi.svg" alt="" width="40" height="40"><div><strong>露米的小叮咛</strong><p>${tab === 'compress' ? '变小之前，先守住细节。<br>像素、透明度和显示相关元数据，都要认真核对。' : '画面看起来一样，不代表数据完全一样。每种格式都有自己的边界。'}</p></div></div></aside>`;
}

function resultPanel() {
  if (!state.result) return '';
  const kind = state.resultKind;
  return `<section class="panel result-panel result-${kind}" tabindex="-1" aria-label="压缩结果示例"><div class="result-top"><span class="success-icon">${icon(kind === 'failure' ? 'info' : 'check')}</span><div><h3>${kind === 'success' ? '轻了一点，细节都在。' : kind === 'unchanged' ? '已经很精简，保留原内容。' : '验证未通过，原文件安然无恙。'}</h3><p>山间来信.png 的${kind === 'success' ? '成功' : kind === 'unchanged' ? '无收益' : '失败'}状态示例，并非真实处理结果</p></div><span class="sample-tag">示例数据</span></div>
    ${kind === 'success' ? `<div class="result-numbers"><div><small>原始大小</small><strong>2.40 MB</strong></div>${icon('arrow')}<div><small>优化后</small><strong>1.68 MB</strong></div><span class="result-saving">节省 30%</span></div><div class="verification-list">${['原始像素一致', '透明像素 RGB 保留', '显示相关元数据保留', '原文件未改动'].map((text) => `<span>${icon('check')}${text}</span>`).join('')}</div><p class="helper">正式结果由解码验证产生，不能仅凭预览判断无损。本稿不会生成可下载的优化文件。</p>` : `<p class="helper">${kind === 'unchanged' ? '没有找到更小且通过验证的候选。正式模式会输出原内容，节省 0%，不将它记为处理错误。' : '示例原因：候选图片的显示相关元数据与输入不一致。正式模式会拒绝发布；可以保留原文件，或查看详细报告后重试。'}</p>`}
    <details><summary>预览其他结果状态</summary><div class="drop-actions"><button class="text-button" data-result="success">压缩成功</button><button class="text-button" data-result="unchanged">没有变小</button><button class="text-button" data-result="failure">验证失败</button></div></details>
  </section>`;
}

function renderImage() {
  main.innerHTML = `<header class="tool-heading"><img src="assets/lumi.svg" alt="露米" width="78" height="78"><div><h1>图片处理</h1><p>Lumi 露米，你的光影师。把每一张图片，整理得刚刚好。</p></div><span class="partner-bubble">小一点的体积，一样认真的细节。</span></header><nav class="tabs" aria-label="图片处理能力">${[['compress', '压缩 PNG'], ['convert', '格式转换'], ['resize', '调整尺寸']].map(([id, label]) => `<button class="tab ${state.imageTab === id ? 'active' : ''}" data-tab="${id}" aria-pressed="${state.imageTab === id}">${label}</button>`).join('')}</nav><div class="image-layout"><div>${filePanel()}${resultPanel()}</div>${optionsPanel()}</div>`;
  const fileInput = document.querySelector('#image-file');
  fileInput.addEventListener('change', () => acceptFile(fileInput.files[0]));
  const dropzone = document.querySelector('#dropzone');
  if (dropzone) {
    dropzone.addEventListener('dragover', (event) => { event.preventDefault(); dropzone.classList.add('dragover'); });
    dropzone.addEventListener('dragleave', () => dropzone.classList.remove('dragover'));
    dropzone.addEventListener('drop', (event) => { event.preventDefault(); dropzone.classList.remove('dragover'); if (event.dataTransfer.files.length !== 1) { state.error = '这份设计稿每次预览一张图片，请选择单个 PNG 文件。'; renderImage(); return; } acceptFile(event.dataTransfer.files[0]); });
  }
}

function renderOverview(tool) {
  main.innerHTML = `<header class="overview-hero" style="${themeStyle(tool)}">${tool.asset ? `<img src="assets/${tool.asset}.svg" alt="${tool.partner}" width="136" height="136">` : '<div class="text-mascot" aria-hidden="true">Aa</div>'}<div><small>${tool.partner}${tool.title ? ` · ${tool.title}` : ''}</small><h1>${tool.name}</h1><p>${tool.description}。<br>选好要做的事，再一步步完成。</p></div></header><div class="section-heading"><h2>能帮你做这些<span>来自现有 CLI 的能力</span></h2><span>能力入口概念页</span></div><section class="capability-grid" aria-label="工具能力">${tool.abilities.map(([name, description]) => `<article class="capability"><h3>${name}</h3><p>${description}</p></article>`).join('')}</section><div class="scope-note">${icon('info')}<p>${tool.boundary}</p><p>这份首轮设计稿展示各领域入口；完整可点击流程集中在「图片处理 → 压缩 PNG」。此页尚未提供操作表单。</p><a class="text-button" href="#image">体验图片处理交互 ${icon('arrow')}</a></div>`;
}

function route({ focus = false } = {}) {
  const id = location.hash.slice(1).split('/')[0];
  state.page = tools.some((tool) => tool.id === id) ? id : 'home';
  document.querySelector('#breadcrumb').textContent = state.page === 'home' ? '工具小队' : `工具小队 / ${tools.find((tool) => tool.id === state.page).name}`;
  renderNavigation();
  if (state.page === 'home') renderHome();
  else if (state.page === 'image') renderImage();
  else renderOverview(tools.find((tool) => tool.id === state.page));
  document.title = `${state.page === 'home' ? '工具小队' : tools.find((tool) => tool.id === state.page).name} · Kito 奇托`;
  setMenu(false);
  document.querySelectorAll('dialog[open]').forEach((dialog) => dialog.close());
  if (focus) { main.focus({ preventScroll: true }); window.scrollTo(0, 0); }
}

// 移动端抽屉不只是移出画面：同步可访问性与焦点，避免 Tab 进入看不见的导航。
function setMenu(open, moveFocus = false) {
  const mobile = matchMedia('(max-width: 720px)').matches;
  const expanded = mobile && open;
  document.body.classList.toggle('menu-open', expanded);
  document.querySelector('.nav-backdrop').hidden = !expanded;
  document.querySelector('.menu-toggle').setAttribute('aria-expanded', String(expanded));
  document.querySelector('.sidebar').inert = mobile && !expanded;
  document.querySelector('.app-body').inert = expanded;
  if (moveFocus) {
    const destination = expanded ? document.querySelector('.sidebar .brand') : document.querySelector('.menu-toggle');
    destination.focus({ preventScroll: true });
  }
}

function focusResult() {
  const result = document.querySelector('.result-panel');
  result.focus({ preventScroll: true });
  result.scrollIntoView({ block: 'nearest', behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'instant' : 'smooth' });
}

function releaseFile() {
  // 撤销未完成的读文件回调，防止快速切换示例后旧回调覆盖新状态。
  fileReadVersion += 1;
  if (state.objectUrl) URL.revokeObjectURL(state.objectUrl);
  state.objectUrl = null;
  state.file = null;
}

async function acceptFile(file) {
  if (!file) return;
  releaseFile();
  const version = fileReadVersion;
  state.sample = false;
  state.result = false;
  state.error = '';
  if (file.size > 20 * 1024 * 1024) { state.error = '设计稿的本地预览限制为 20 MB。请换一张更小的 PNG 图片；此限制不代表 CLI 的处理能力。'; renderImage(); return; }
  try {
    const signature = new Uint8Array(await file.slice(0, 8).arrayBuffer());
    if (version !== fileReadVersion) return;
    const pngSignature = [137, 80, 78, 71, 13, 10, 26, 10];
    if (!pngSignature.every((byte, index) => signature[index] === byte)) throw new Error('请选择 PNG 图片。JPEG、WebP 等格式请使用「格式转换」；当前原型仅预览 PNG。');
    const url = URL.createObjectURL(file);
    state.objectUrl = url;
    const preview = new Image();
    preview.src = url;
    await preview.decode();
    if (version !== fileReadVersion) return;
    state.file = { name: file.name, size: `${(file.size / (1024 * 1024)).toFixed(2)} MB` };
  } catch (error) {
    if (version !== fileReadVersion) return;
    releaseFile();
    state.error = error instanceof Error && error.message.startsWith('请选择') ? error.message : '无法预览这张图片。请检查文件是否完整，或改用示例体验。';
  }
  if (state.page === 'image') renderImage();
}

function showInfo(kind = 'about') {
  document.querySelector('#info-title').textContent = kind === 'principles' ? '让工具好用，也让人安心' : 'Kito，换一种顺手的打开方式';
  document.querySelector('#info-content').innerHTML = `<p>各有所长，一起把事情做好。这份 Web UI 概念稿把 Kito 的工具小队带到浏览器，让不熟悉命令行的人也能从具体任务开始。</p><h3>这次可以体验什么</h3><ul><li>八个工具领域的导航、搜索与能力介绍。</li><li>图片页的选图、拖入、本地预览、优化方式，以及三类结果示例。</li><li>桌面和窄屏布局、键盘导航与减少动态效果偏好。</li></ul><h3>设计选择</h3><p>暖黄代表「动手做」，柔和底色让内容更清楚；七位几何伙伴各有轮廓与颜色。功能名称先行，不必记住角色设定，也能找到工具。</p><div class="swatches" aria-label="品牌配色"><span style="background:#f4cf66"></span><span style="background:#a9d4c0"></span><span style="background:#b7a6de"></span><span style="background:#e5a082"></span><span style="background:#9abcdc"></span></div><h3>从设计到产品</h3><p>预期通过 <code>kito web</code> 打开 Kito 网页版。此命令与 Web 后端尚未实现，本稿不调用 CLI、不上传文件，也不生成处理结果。后续需接入本机文件选择、后端能力检查、任务执行和验证报告。</p><p>文本工具已存在于 CLI，品牌文档尚未为它设定伙伴，因此本稿保留功能入口，不新造角色。</p>`;
  document.querySelector('#info-dialog').showModal();
}

function renderSearch(query = '') {
  const value = query.trim().toLocaleLowerCase();
  const matches = tools.filter((tool) => `${tool.name} ${tool.partner} ${tool.description} ${tool.id} ${tool.abilities.flat().join(' ')}`.toLocaleLowerCase().includes(value));
  const results = document.querySelector('#search-results');
  results.innerHTML = matches.length ? matches.map((tool) => `<a href="#${tool.id}" class="search-result">${tool.asset ? `<img src="assets/${tool.asset}.svg" alt="">` : icon('text')}<div><strong>${tool.name}</strong><small>${tool.description}</small></div>${icon('arrow')}</a>`).join('') : '<p class="helper" role="status">还没找到这个工具。试试「图片」「时间」或伙伴的名字。</p>';
}

function toast(message) {
  const element = document.querySelector('#toast');
  element.textContent = message;
  element.classList.add('visible');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => element.classList.remove('visible'), 3500);
}

document.addEventListener('click', (event) => {
  if (event.target.closest('.skip-link')) { event.preventDefault(); main.focus(); return; }
  const target = event.target.closest('[data-action], [data-tab], [data-result], .search-result');
  if (!target) return;
  if (target.matches('.search-result')) { document.querySelector('#search-dialog').close(); if (target.hash === location.hash) main.focus(); return; }
  if (target.dataset.tab) { state.imageTab = target.dataset.tab; state.result = false; renderImage(); document.querySelector(`[data-tab="${state.imageTab}"]`).focus(); return; }
  if (target.dataset.result) { state.resultKind = target.dataset.result; renderImage(); focusResult(); return; }
  switch (target.dataset.action) {
    case 'about': case 'principles': showInfo(target.dataset.action); break;
    case 'search': renderSearch(); document.querySelector('#tool-search').value = ''; document.querySelector('#search-dialog').showModal(); document.querySelector('#tool-search').focus(); break;
    case 'close': target.closest('dialog').close(); break;
    case 'menu': setMenu(!document.body.classList.contains('menu-open'), true); break;
    case 'choose-file': document.querySelector('#image-file').click(); break;
    case 'sample': releaseFile(); state.sample = true; state.result = false; state.error = ''; renderImage(); document.querySelector('[data-action="demo"]').focus({ preventScroll: true }); break;
    case 'remove-file': releaseFile(); state.sample = false; state.result = false; state.error = ''; renderImage(); document.querySelector('[data-action="choose-file"]').focus({ preventScroll: true }); break;
    case 'demo':
      if (state.imageTab !== 'compress') { toast('参数布局预览；完整结果交互请切回「压缩 PNG」。'); return; }
      state.result = true;
      state.resultKind = 'success';
      renderImage();
      focusResult();
      toast('正在展示固定示例，未处理任何文件。');
      break;
  }
});

document.addEventListener('change', (event) => {
  if (event.target.name === 'effort') state.effort = event.target.value;
  if (event.target.id === 'format-choice') {
    const extension = ['webp', 'png', 'tiff'][event.target.selectedIndex];
    const sourceName = state.sample ? '山间来信' : state.file ? state.file.name.replace(/\.png$/i, '') : '图片名称';
    document.querySelector('#output-name').textContent = `${sourceName}.${extension}`;
  }
});
document.querySelector('#tool-search').addEventListener('input', (event) => renderSearch(event.target.value));
document.addEventListener('keydown', (event) => {
  // search 输入框默认会先用 Escape 清空文字；在弹窗中优先遵循「Esc 关闭」。
  if (event.key === 'Escape' && document.querySelector('dialog[open]')) {
    event.preventDefault();
    document.querySelector('dialog[open]').close();
    return;
  }
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); if (!document.querySelector('dialog[open]')) document.querySelector('[data-action="search"]').click(); }
  if (document.body.classList.contains('menu-open')) {
    if (event.key === 'Escape') { setMenu(false, true); return; }
    if (event.key === 'Tab') {
      const items = [...document.querySelectorAll('.sidebar a, .sidebar button')];
      const first = items[0], last = items.at(-1);
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    }
  }
});
window.addEventListener('hashchange', () => route({ focus: true }));
window.addEventListener('beforeunload', releaseFile);
matchMedia('(max-width: 720px)').addEventListener('change', () => setMenu(false));
hydrateIcons();
route();
