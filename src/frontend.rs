use axum::{
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
};

pub async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

pub async fn css() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        APP_CSS,
    )
        .into_response()
}

pub async fn js() -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        APP_JS,
    )
        .into_response()
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>高考志愿填报助手</title>
  <link rel="stylesheet" href="/app.css" />
  <link rel="stylesheet" href="/lib/vis-9.1.2/vis-network.css" />
  <script src="/lib/vis-9.1.2/vis-network.min.js"></script>
</head>
<body>
  <aside class="sidebar">
    <div class="brand">
      <img src="/assets/orange.png" alt="小橘" />
      <div>
        <strong>小橘助手</strong>
        <span>高考志愿填报系统</span>
      </div>
    </div>
    <nav id="nav"></nav>
    <p class="side-note">图片来源于网络，测试结果仅供参考</p>
  </aside>
  <main class="main">
    <section id="toast" class="toast hidden"></section>
    <section id="app"></section>
  </main>
  <script src="/app.js"></script>
</body>
</html>"#;

const APP_CSS: &str = r#"
:root {
  --orange: #ff914d;
  --blue: #4f8df7;
  --text: #263238;
  --muted: #667085;
  --bg: linear-gradient(135deg, #f5f7fa 0%, #e4edf5 100%);
  --card: rgba(255,255,255,0.68);
  --shadow: 0 8px 32px rgba(31,38,135,0.15);
}
* { box-sizing: border-box; }
body {
  margin: 0;
  min-height: 100vh;
  color: var(--text);
  background: var(--bg);
  font-family: "Microsoft YaHei", "PingFang SC", system-ui, sans-serif;
}
.sidebar {
  position: fixed;
  inset: 0 auto 0 0;
  width: 280px;
  padding: 24px 18px;
  background: rgba(255,255,255,0.82);
  border-right: 1px solid rgba(255,255,255,0.8);
  box-shadow: 4px 0 26px rgba(31,38,135,0.08);
  overflow-y: auto;
}
.brand { display: flex; align-items: center; gap: 12px; margin-bottom: 24px; }
.brand img { width: 54px; height: 54px; border-radius: 50%; }
.brand strong { display: block; color: var(--orange); font-size: 22px; }
.brand span { display: block; color: var(--muted); font-size: 13px; margin-top: 3px; }
.nav-group { margin: 20px 0 10px; color: #344054; font-weight: 700; font-size: 14px; }
.nav-btn {
  width: 100%;
  margin: 4px 0;
  padding: 10px 12px;
  border: 0;
  border-radius: 12px;
  color: #344054;
  background: transparent;
  text-align: left;
  cursor: pointer;
  transition: .18s ease;
}
.nav-btn:hover, .nav-btn.active { background: #fff3ea; color: #e86819; }
.side-note { margin-top: 36px; color: var(--muted); font-size: 13px; line-height: 1.6; }
.main { margin-left: 280px; padding: 32px min(5vw, 70px); }
.page { max-width: 1120px; margin: 0 auto; }
h1, h2, h3 { color: var(--blue); }
h1 { text-align: center; font-size: clamp(28px, 4vw, 44px); margin: 18px 0 18px; }
h2 { text-align: center; margin: 12px 0 20px; }
.glass-card {
  background: var(--card);
  border-radius: 18px;
  padding: 28px 34px;
  margin: 22px 0;
  box-shadow: var(--shadow);
  backdrop-filter: blur(8px);
  border: 1px solid rgba(255,255,255,0.55);
}
.grid { display: grid; gap: 18px; }
.grid-2 { grid-template-columns: repeat(2, minmax(0, 1fr)); }
.grid-main { grid-template-columns: minmax(0,1fr) 300px; align-items: start; }
.field { display: grid; gap: 7px; margin: 12px 0; }
label { font-weight: 600; color: #344054; }
input, select, textarea {
  width: 100%;
  padding: 11px 13px;
  border: 1px solid #d0d5dd;
  border-radius: 11px;
  background: rgba(255,255,255,.92);
  font: inherit;
}
textarea { min-height: 92px; resize: vertical; }
.btn {
  display: inline-flex;
  justify-content: center;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  border: 0;
  border-radius: 12px;
  color: white;
  background: var(--blue);
  font-weight: 700;
  cursor: pointer;
  min-height: 42px;
}
.btn.secondary { background: #667085; }
.btn.orange { background: var(--orange); }
.btn.ghost { color: var(--blue); background: white; border: 1px solid #d0d5dd; }
.orange-btn {
  width: 58px;
  height: 58px;
  border: 0;
  border-radius: 50%;
  background: url("/assets/orange.png") center / cover no-repeat;
  cursor: pointer;
  filter: drop-shadow(0 4px 8px rgba(255,145,77,.35));
}
.message { padding: 12px 14px; border-radius: 12px; margin: 12px 0; }
.success { background: #ecfdf3; color: #027a48; }
.error { background: #fef3f2; color: #b42318; }
.warning { background: #fffaeb; color: #b54708; }
.info { background: #eff8ff; color: #175cd3; }
.check-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0.5rem;
  margin-top: 0.35rem;
}
.check-grid label {
  display: flex;
  gap: 0.45rem;
  align-items: center;
  padding: 0.5rem 0.7rem;
  border-radius: 999px;
  border: 1px solid #fed7aa;
  background: #fff7ed;
  color: #7c2d12;
  font-weight: 650;
}
.check-grid input { width: auto; }
.table-wrap { overflow-x: auto; }
table { width: 100%; border-collapse: collapse; background: white; border-radius: 12px; overflow: hidden; }
th, td { padding: 10px 12px; border-bottom: 1px solid #eaecf0; text-align: left; }
th { background: #f9fafb; color: #344054; }
.hero-img { width: 100%; border-radius: 18px; box-shadow: var(--shadow); }
.prompt-row { display:flex; gap: 20px; align-items:center; justify-content:center; flex-wrap: wrap; }
.action-row { display:flex; gap: 18px; align-items:stretch; justify-content:center; flex-wrap: wrap; margin: 28px 0 8px; }
.action-card {
  min-width: 240px;
  border: 0;
  border-radius: 18px;
  padding: 16px 20px;
  background: white;
  color: var(--blue);
  box-shadow: 0 8px 26px rgba(31,38,135,.10);
  cursor: pointer;
  display: flex;
  gap: 14px;
  align-items: center;
  text-align: left;
  transition: transform .18s ease, box-shadow .18s ease;
}
.action-card:hover { transform: translateY(-2px); box-shadow: 0 12px 30px rgba(31,38,135,.16); }
.action-card.primary { background: linear-gradient(135deg, #ff914d, #ffb067); color: white; }
.action-card img { width: 54px; height: 54px; flex: 0 0 auto; filter: drop-shadow(0 4px 8px rgba(255,145,77,.25)); }
.action-card strong { display:block; font-size: 1.05rem; margin-bottom: 4px; }
.action-card span { display:block; font-size: .88rem; opacity: .86; line-height: 1.45; }
.center { text-align: center; }
.progress { height: 12px; border-radius: 99px; background: #eaecf0; overflow: hidden; }
.progress span { display:block; height:100%; background: var(--orange); transition: width .2s; }
.chat-log { display: grid; gap: 10px; margin-bottom: 16px; }
.chat-msg { display:flex; gap: 10px; align-items:flex-start; }
.chat-msg img { width: 36px; height: 36px; border-radius: 50%; }
.bubble { padding: 12px 14px; border-radius: 16px; background: white; box-shadow: 0 3px 12px rgba(31,38,135,.08); max-width: 820px; white-space: pre-wrap; }
.user .bubble { background:#eef4ff; }
.markdown {
  line-height: 1.75;
  white-space: normal;
}
.markdown h2, .markdown h3 {
  text-align: left;
  margin: 0.85rem 0 0.45rem;
  color: #2563eb;
}
.markdown h2 { font-size: 1.35rem; }
.markdown h3 { font-size: 1.12rem; }
.markdown p { margin: 0.45rem 0 0.75rem; }
.markdown ul, .markdown ol { margin: 0.45rem 0 0.85rem 1.35rem; padding: 0; }
.markdown li { margin: 0.28rem 0; }
.markdown strong { color: #0f172a; }
.ai-summary {
  border-left: 6px solid var(--orange);
  background: linear-gradient(135deg, #fff7ed, #eff8ff);
}
.kg-box { height: 620px; background: white; border-radius: 16px; border: 1px solid #eaecf0; }
.toast {
  position: fixed; top: 22px; right: 28px; z-index: 10;
  background: #111827; color: white; padding: 12px 16px; border-radius: 12px;
  box-shadow: var(--shadow);
}
.hidden { display: none !important; }
@media (max-width: 860px) {
  .sidebar { position: static; width: auto; }
  .main { margin-left: 0; padding: 18px; }
  .grid-2, .grid-main { grid-template-columns: 1fr; }
}
"#;

const APP_JS: &str = r#"
const API = "";
localStorage.removeItem("loggedIn");
localStorage.removeItem("currentUser");
localStorage.removeItem("role");
const state = {
  loggedIn: sessionStorage.getItem("loggedIn") === "true",
  currentUser: sessionStorage.getItem("currentUser") || "",
  page: location.hash?.slice(1) || "home",
  verification: {},
  student: null,
  questions: [],
  answers: {},
  mbtiPage: 0,
  chatHistory: [],
};

const pages = [
  ["注册&登录", [["login", "注册"]]],
  ["志愿填报推荐", [["home", "主页"]]],
  ["专业探索工具", [["mbti", "MBTI|专业生成器"], ["mbti-test-start", "MBTI|测试入口"], ["mbti-test", "MBTI|性格测试"], ["kg", "知识图谱"]]],
  ["获取志愿结果", [["result", "获取志愿结果"]]],
];

function $(id) { return document.getElementById(id); }
function html(strings, ...values) { return strings.map((s, i) => s + (values[i] ?? "")).join(""); }
function escapeHtml(v) { return String(v ?? "").replace(/[&<>"']/g, s => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#039;'}[s])); }
function showToast(text) { const el = $("toast"); el.textContent = text; el.classList.remove("hidden"); setTimeout(()=>el.classList.add("hidden"), 2600); }
async function api(path, options = {}) {
  const res = await fetch(API + path, { headers: { "Content-Type": "application/json; charset=utf-8", ...(options.headers || {}) }, ...options });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}
function setPage(page) { state.page = page; location.hash = page; render(); }
window.addEventListener("hashchange", () => { state.page = location.hash.slice(1) || "home"; render(); });

function renderNav() {
  $("nav").innerHTML = pages.map(([group, items]) => html`
    <div class="nav-group">${group}</div>
    ${items.map(([id, title]) => `<button class="nav-btn ${state.page===id?'active':''}" onclick="setPage('${id}')">${title}</button>`).join("")}
  `).join("") + (state.loggedIn ? `<div class="side-note">当前用户：${escapeHtml(state.currentUser)}<br/><button class="btn ghost" onclick="logout()">退出登录</button></div>` : "");
}

function requireLogin() {
  if (state.loggedIn) return false;
  $("app").innerHTML = `<div class="page"><div class="message warning">请先登录以使用系统功能</div><button class="btn" onclick="setPage('login')">前往登录页面</button></div>`;
  return true;
}

function render() {
  renderNav();
  if (!state.loggedIn && state.page !== "login") return requireLogin();
  const table = {
    login: loginPage, home: homePage, "mbti": mbtiHomePage, "mbti-test-start": mbtiStartPage,
    "mbti-test": mbtiTestPage, kg: kgPage, result: resultPage,
  };
  (table[state.page] || homePage)();
}

function loginPage(mode = "login") {
  $("app").innerHTML = html`
    <div class="page">
      <h1>小橘助手--专业的大学志愿填报助手</h1>
      <h2>${mode === "register" ? "注册" : mode === "admin" ? "管理员模式" : "登录"}</h2>
      <div class="glass-card" id="login-card"></div>
    </div>`;
  if (mode === "register") return renderRegister();
  if (mode === "admin") return renderAdmin();
  $("login-card").innerHTML = html`
    <div class="grid grid-2">
      <button class="btn" onclick="renderPwdLogin()">🔑 密码登录</button>
      <button class="btn ghost" onclick="renderCodeLogin()">📱 验证码登录</button>
    </div>
    <div id="login-mode"></div>
    <div class="grid grid-2">
      <button class="btn orange" onclick="loginPage('register')">📝 立即注册</button>
      <button class="btn secondary" onclick="loginPage('admin')">🔒 管理员模式</button>
    </div>`;
  renderPwdLogin();
}
function renderPwdLogin() {
  $("login-mode").innerHTML = html`
    <h3>密码登录</h3>
    <div class="field"><label>手机号</label><input id="login-phone" /></div>
    <div class="field"><label>密码</label><input id="login-pwd" type="password" /></div>
    <button class="btn" onclick="loginByPassword()">🔐 登录</button>`;
}
function renderCodeLogin() {
  $("login-mode").innerHTML = html`
    <h3>验证码登录</h3>
    <div class="field"><label>手机号</label><input id="code-phone" /></div>
    <div class="field"><label>验证码</label><input id="code-value" /></div>
    <div class="grid grid-2"><button class="btn ghost" onclick="sendCode()">📤 获取验证码</button><button class="btn" onclick="loginByCode()">✅ 登录</button></div>`;
}
async function loginByPassword() {
  const body = { phone_number: $("login-phone").value.trim(), password: $("login-pwd").value };
  const res = await api("/api/orange/loader", { method: "POST", body: JSON.stringify(body) });
  if (res.state === 200) completeLogin(body.phone_number); else showToast(res.message);
}
function sendCode() {
  const phone = ($("code-phone")?.value || $("reg-phone")?.value || "").trim();
  if (!/^\d{11}$/.test(phone)) return showToast("请输入11位手机号");
  const code = String(Math.floor(100000 + Math.random() * 900000));
  state.verification[phone] = code;
  showToast(`验证码已发送（模拟：${code}）`);
}
function loginByCode() {
  const phone = $("code-phone").value.trim();
  const code = $("code-value").value.trim();
  if (state.verification[phone] !== code) return showToast("验证码错误或已过期");
  completeLogin(phone);
}
function completeLogin(phone) {
  state.loggedIn = true; state.currentUser = phone;
  sessionStorage.setItem("loggedIn", "true"); sessionStorage.setItem("currentUser", phone);
  setPage("home");
}
function logout() { localStorage.clear(); sessionStorage.clear(); state.loggedIn=false; state.currentUser=""; setPage("login"); }
function renderRegister() {
  $("login-card").innerHTML = html`
    <div class="field"><label>📱 手机号</label><input id="reg-phone" /></div>
    <div class="field"><label>🔑 密码</label><input id="reg-pwd" type="password" /></div>
    <div class="field"><label>🔑 确认密码</label><input id="reg-pwd2" type="password" /></div>
    <div class="field"><label>验证码</label><input id="reg-code" /></div>
    <div class="grid grid-2"><button class="btn ghost" onclick="sendCode()">📤 获取验证码</button><button class="btn orange" onclick="registerUser()">🎯 立即注册</button></div>
    <p><button class="btn secondary" onclick="loginPage()">🔙 返回登录</button></p>`;
}
async function registerUser() {
  const phone = $("reg-phone").value.trim(), pwd = $("reg-pwd").value, pwd2 = $("reg-pwd2").value, code = $("reg-code").value.trim();
  if (!/^1[3-9]\d{9}$/.test(phone)) return showToast("手机号格式不正确");
  if (!pwd || pwd !== pwd2) return showToast("请确认两次密码一致");
  if (state.verification[phone] !== code) return showToast("验证码错误或已过期");
  const res = await api("/api/orange/register", { method: "POST", body: JSON.stringify({ phone_number: phone, password: pwd }) });
  if (res.state === 200) completeLogin(phone); else showToast(res.message);
}
function renderAdmin() {
  $("login-card").innerHTML = html`
    <div class="field"><label>管理员账号</label><input id="admin-phone" /></div>
    <div class="field"><label>管理员密码</label><input id="admin-pwd" type="password" /></div>
    <button class="btn" onclick="adminLook()">以管理员身份登录👨‍💻</button>
    <button class="btn secondary" onclick="loginPage()">返回登录页面</button>
    <div id="admin-result"></div>`;
}
async function adminLook() {
  const res = await api("/api/orange/lookat", { method:"POST", body: JSON.stringify({ phone_number:$("admin-phone").value, password:$("admin-pwd").value }) });
  if (res.state !== 200) return $("admin-result").innerHTML = `<div class="message warning">${escapeHtml(res.message)}</div>`;
  state.loggedIn = true; state.currentUser = $("admin-phone").value.trim() || "admin";
  sessionStorage.setItem("loggedIn", "true"); sessionStorage.setItem("currentUser", state.currentUser);
  sessionStorage.setItem("role", "admin");
  $("admin-result").innerHTML = `<div class="message success">管理员登录成功，可以正常使用主页、MBTI、知识图谱和志愿推荐功能。</div>
    <h3>当前用户列表</h3>${tableHtml(["手机号","密码"], res.message)}
    <p><button class="btn orange" onclick="setPage('home')">进入系统主页</button></p>`;
}

function homePage() {
  $("app").innerHTML = html`
    <div class="page grid grid-main">
      <div>
        <h1>请完善您的信息(ง๑ •̀_•́)ง</h1>
        <div class="glass-card">
          <div class="prompt-row"><img src="/assets/clx.png" width="80" /><span>填好后请点击最下方橘子提交</span></div>
          <div class="grid grid-2">
            ${field("province","所在省份", "select", provinces(), "天津市")}
            ${field("score","高考总分", "number", "", "560")}
            ${field("rank","全省排名", "number", "", "12000")}
            ${subjectsField()}
          </div>
          ${field("want","感兴趣专业🍊", "text", "", "计算机")}
          ${field("unwant","不感兴趣专业🍊", "text", "", "无")}
          ${field("goal","职业发展目标🍊", "text", "", "软件工程师")}
          ${field("strategy","志愿填报策略🍊", "select", ["科目优先","城市优先","院校优先"], "科目优先")}
          ${field("city","偏好城市？", "text", "", "")}
          ${field("hobby","兴趣爱好🍊", "text", "", "编程")}
          <p class="center"><button class="orange-btn" title="提交" onclick="submitStudent()"></button></p>
        </div>
        <details class="glass-card"><summary>还不清楚自己的排名？点击查看一分一段表🍊</summary><div id="score-table">加载中...</div></details>
      </div>
      <div><img id="home-hero" class="hero-img" src="/assets/home0.png" /></div>
    </div>`;
  loadScoreDistribution();
  startHero();
}
function field(id,label,type,options,value="") {
  if (type === "select") return `<div class="field"><label>${label}</label><select id="${id}">${options.map(o=>`<option ${o===value?"selected":""}>${o}</option>`).join("")}</select></div>`;
  return `<div class="field"><label>${label}</label><input id="${id}" type="${type}" value="${escapeHtml(value)}" /></div>`;
}
function subjectsField() {
  const subjects = ["物理", "化学", "生物", "政治", "历史", "地理", "技术"];
  return `<div class="field"><label>选考科目（最多选择 3 门）</label><div class="check-grid">${subjects.map((s, i) =>
    `<label><input type="checkbox" name="subject" value="${s}" ${i < 3 ? "checked" : ""}>${s}</label>`
  ).join("")}</div></div>`;
}
function provinces(){return ["北京市","天津市","河北省","山西省","内蒙古自治区","辽宁省","吉林省","黑龙江省","上海市","江苏省","浙江省","安徽省","福建省","江西省","山东省","河南省","湖北省","湖南省","广东省","广西壮族自治区","海南省","重庆市","四川省","贵州省","云南省","西藏自治区","陕西省","甘肃省","青海省","宁夏回族自治区","新疆维吾尔自治区"];}
async function loadScoreDistribution() {
  try { const res = await api("/api/orange/score_distribution"); $("score-table").innerHTML = objectTable(res.rows.slice(0, 40)); }
  catch(e){ $("score-table").innerHTML = `<div class="message error">数据库连接失败：${escapeHtml(e.message)}</div>`; }
}
function startHero() {
  const el = $("home-hero"); if (!el) return;
  let flip = false; clearInterval(window.heroTimer);
  window.heroTimer = setInterval(()=>{ if($("home-hero")) { flip=!flip; $("home-hero").src = flip ? "/assets/home1.png" : "/assets/home0.png"; } }, 5000);
}
async function submitStudent() {
  const selectedSubjects = [...document.querySelectorAll("input[name=subject]:checked")].map(el => el.value);
  if (selectedSubjects.length === 0 || selectedSubjects.length > 3) {
    showToast("请选择 1 到 3 门选考科目");
    return;
  }
  const profile = {
    score:$("score").value, live_city:$("province").value, rank:$("rank").value, want_major:$("want").value || "无",
    unwant_major:$("unwant").value || "无", hobby:$("hobby").value || "无", future_goal:$("goal").value || "无",
    strategy:$("strategy").value === "城市优先" && $("city").value ? `城市优先:${$("city").value}` : $("strategy").value,
    subjects:selectedSubjects.join(","),
  };
  state.student = profile;
  $("app").innerHTML = `<div class="page"><h2>确认信息</h2><div class="glass-card">${objectTable([{
    所在省份:profile.live_city, 高考总分:profile.score, 选考科目:profile.subjects, 全省排名:profile.rank,
    感兴趣专业:profile.want_major, 不感兴趣专业:profile.unwant_major, 职业发展目标:profile.future_goal, 志愿填报策略:profile.strategy, 兴趣爱好:profile.hobby
  }])}<p><button class="btn" onclick="confirmStudent()">确认信息无误，提交</button> <button class="btn secondary" onclick="homePage()">返回上一页</button></p></div></div>`;
}
async function confirmStudent() {
  try {
    await api("/api/orange/student", { method:"POST", body: JSON.stringify(state.student) });
    showToast("信息已提交，正在获取推荐结果...");
    await api("/api/orange/smart_recommend", { method:"POST", body:"{}" });
    showToast("已获取到院校推荐结果");
    setPage("result");
  } catch(e) { showToast("发送失败：" + e.message); }
}

function mbtiHomePage() {
  $("app").innerHTML = html`<div class="page"><h1>MBTI专业生成器</h1><div class="glass-card">
    <p>MBTI专业生成器是一款基于国际公认的MBTI性格类型理论开发的智能职业推荐系统。通过科学分析您的性格特质，我们能够为您匹配最适合的专业发展方向。</p>
    <p>我们的系统采用先进的匹配算法，结合职业数据，确保推荐结果的准确性和实用性。所有用户数据严格加密，测试过程完全匿名。</p>
    ${field("mbti-input","请输入您的MBTI类型（如 INTJ、ENFP）","text","","INTJ")}
    <div class="action-row">
      <button class="action-card primary" onclick="seekMbti()" title="根据上方输入的 MBTI 类型生成职业和专业方向推荐">
        <img src="/assets/orange.png" alt="" />
        <span><strong>生成专业推荐</strong>已知道自己的 MBTI，点击查看适合的职业与专业方向</span>
      </button>
      <button class="action-card" onclick="setPage('mbti-test-start')" title="不知道 MBTI 时，进入 40 题 MBTI 性格测试">
        <img src="/assets/orange.png" alt="" />
        <span><strong>不知道 MBTI？去做测试</strong>进入 40 题小橘速测，测完自动生成推荐</span>
      </button>
    </div>
    <div id="mbti-result"></div>
  </div></div>`;
}
async function seekMbti() {
  const type = $("mbti-input").value.trim().toUpperCase();
  if (!/^[IE][NS][FT][JP]$/.test(type)) return showToast("请输入有效的MBTI类型！");
  $("mbti-result").innerHTML = `<div class="message info">您的MBTI类型是：${type}。正在为您匹配最适合的职业...</div>`;
  const res = await api("/api/orange/seek", { method:"POST", body: JSON.stringify({mbti_type:type}) });
  typeText("mbti-result", res.description);
}
function mbtiStartPage() {
  $("app").innerHTML = `<div class="page"><h1>MBTI 性格测试</h1><div class="glass-card">
    <p>📊 基于Myers-Briggs理论开发的40题专业量表，帮助你了解认知偏好和行为模式。</p>
    <p>🔍 通过40道题目，深入了解你的性格特质，帮助你更好地规划专业发展和人际关系。</p>
    <p>👇 点击下方小橘开启测试。</p><p class="center"><button class="orange-btn" onclick="startMbtiTest()"></button></p></div></div>`;
}
async function startMbtiTest() {
  const res = await api("/api/orange/questions", { method:"POST", body:"{}" });
  state.questions = res.question; state.answers = {}; state.mbtiPage = 0; setPage("mbti-test");
}
function mbtiTestPage() {
  if (!state.questions.length) return mbtiStartPage();
  const start = state.mbtiPage * 5, end = Math.min(start + 5, state.questions.length);
  $("app").innerHTML = `<div class="page"><h1>MBTI 性格测试</h1><div class="glass-card"><p>回答以下问题，发现你的性格类型</p>
    ${state.questions.slice(start,end).map((q,offset)=>questionHtml(q,start+offset)).join("")}
    <div class="progress"><span style="width:${Object.keys(state.answers).length/state.questions.length*100}%"></span></div>
    <p>已完成 ${Object.keys(state.answers).length}/${state.questions.length} 题；当前页：${state.mbtiPage+1}/${Math.ceil(state.questions.length/5)}</p>
    <button class="btn secondary" ${state.mbtiPage===0?"disabled":""} onclick="saveAnswers(); state.mbtiPage--; mbtiTestPage()">上一页</button>
    ${end === state.questions.length ? `<button class="btn orange" onclick="submitMbtiTest()">提交</button>` : `<button class="btn" onclick="nextMbtiPage()">下一页</button>`}
    <div id="mbti-test-result"></div></div></div>`;
}
function questionHtml(q, idx) {
  const selected = state.answers[idx] || 0;
  return `<div class="glass-card"><h3>问题 ${idx+1}</h3><p>${escapeHtml(q[0])}</p>
    <label><input type="radio" name="q${idx}" value="1" ${selected===1?"checked":""}/> ${escapeHtml(q[1])}</label><br/>
    <label><input type="radio" name="q${idx}" value="2" ${selected===2?"checked":""}/> ${escapeHtml(q[2])}</label></div>`;
}
function saveAnswers(){ document.querySelectorAll("input[type=radio]:checked").forEach(el => state.answers[el.name.slice(1)] = Number(el.value)); }
function nextMbtiPage(){ saveAnswers(); state.mbtiPage++; mbtiTestPage(); }
async function submitMbtiTest() {
  saveAnswers();
  if (Object.keys(state.answers).length < state.questions.length) return showToast("请完成全部题目");
  const mbti = await api("/api/orange/result", { method:"POST", body: JSON.stringify({ operation: state.answers }) });
  const res = await api("/api/orange/seek", { method:"POST", body: JSON.stringify({ mbti_type: mbti }) });
  $("mbti-test-result").innerHTML = `<div class="message success">您的MBTI类型是：${mbti}。正在为您匹配最适合的职业...</div><div id="career-text"></div><button class="btn ghost" onclick="startMbtiTest()">重新测试</button>`;
  typeText("career-text", res.description);
}

function kgPage() {
  $("app").innerHTML = `<div class="page"><h1>📝 专业分析知识图谱</h1><div class="glass-card">
    <p>你是否对不同专业的具体方向依然模糊不清？你是否对未来的专业选择感到迷茫，不知从何入手？</p>
    <p>别担心，专业知识图谱功能为你指明方向！只需输入你的兴趣领域，我们就能为你梳理出清晰的细分方向。</p>
    <p>✅ 精准推荐：从专业授予门类到具体专业名称一键获取专业细分领域。<br/>✅ 高效决策：告别信息过载，快速锁定研究方向。<br/>✅ 个性化探索：无论理工科还是文史哲经，都能找到属于你的知识地图。</p>
    ${field("kg-input","请输入您的专业偏好（如：我对物理学感兴趣）","text","","我对人工智能和编程感兴趣")}
    <button class="btn orange" onclick="runKg()">获取专业知识图谱🍊</button>
    <div id="kg-text"></div><div id="kg-network" class="kg-box"></div>
  </div></div>`;
}
async function runKg() {
  const text = $("kg-input").value.trim(); if (!text) return showToast("请输入内容再提交！");
  $("kg-text").innerHTML = `<div class="message info">分析中...</div>`;
  const kg = await api("/get_dynamic_kg", { method:"POST", body: JSON.stringify({text, extra:""}) });
  const res = await fetch("/process", { method:"POST", headers:{"Content-Type":"application/json"}, body:JSON.stringify({text}) });
  await renderSseToElement(res, "kg-text");
  drawKg(kg.kg_data || []);
}
function drawKg(rows) {
  const nodeMap = new Map(), nodes = [], edges = [];
  function id(name){ if(!nodeMap.has(name)){ nodeMap.set(name,nodeMap.size); nodes.push({id:nodeMap.get(name), label:name}); } return nodeMap.get(name); }
  rows.forEach(r => { if (r[0] === "实体关系") edges.push({from:id(r[1]), to:id(r[3]), label:r[2], arrows:"to"}); });
  new vis.Network($("kg-network"), { nodes:new vis.DataSet(nodes), edges:new vis.DataSet(edges) }, { physics:{ stabilization:false }, edges:{ font:{align:"middle"} } });
}

async function resultPage() {
  $("app").innerHTML = `<div class="page"><h1>获取您的院校推荐</h1>
    <div class="glass-card ai-summary"><h2>推荐解读 🍊</h2><div id="recommend-summary" class="markdown">加载中...</div></div>
    <div class="glass-card"><details><summary>查看完整志愿表</summary><div id="recommend-table">加载中...</div></details></div>
    <div class="glass-card"><h3>您还有什么想问的吗🍊</h3><div id="chat-log" class="chat-log"></div><div class="grid grid-2"><input id="chat-input" placeholder="输入问题" /><button class="btn orange" onclick="sendChat()">发送</button></div></div></div>`;
  loadRecommendSummary();
  loadRecommend();
  renderChat();
}
async function loadRecommendSummary() {
  try {
    const res = await api("/api/orange/recommend_summary");
    $("recommend-summary").innerHTML = renderMarkdown(res.summary || "暂无推荐解读。");
  } catch(e) {
    $("recommend-summary").innerHTML = `<div class="message info">推荐解读正在准备中，请先在主页提交信息。</div>`;
  }
}
async function loadRecommend() {
  try {
    const raw = await api("/api/orange/recommend_result");
    const data = JSON.parse(raw);
    const rows = [];
    Object.entries(data).forEach(([tier, schools]) => schools.forEach(s => rows.push({"志愿类型":tier, ...s})));
    $("recommend-table").innerHTML = `<p>表格更新时间：${new Date().toLocaleString()}</p>` + objectTable(rows);
  } catch(e) { $("recommend-table").innerHTML = `<div class="message info">数据正在准备中，请先在主页提交信息。</div>`; }
}
function renderChat() {
  $("chat-log").innerHTML = state.chatHistory.map(([u,a]) => `<div class="chat-msg user"><img src="/assets/user.png"/><div class="bubble">${escapeHtml(u)}</div></div><div class="chat-msg"><img src="/assets/orange.png"/><div class="bubble markdown">${renderMarkdown(a)}</div></div>`).join("");
}
async function sendChat() {
  const msg = $("chat-input").value.trim(); if(!msg) return;
  state.chatHistory.push([msg, ""]);
  renderChat();
  const idx = state.chatHistory.length - 1;
  const res = await fetch("/api/orange/chat/stream", { method:"POST", headers:{"Content-Type":"application/json"}, body:JSON.stringify({message:msg, history:state.chatHistory.slice(0,-1)}) });
  const text = await collectSse(res);
  state.chatHistory[idx][1] = text;
  renderChat();
  $("chat-input").value = "";
}

async function renderSseToElement(response, id) { const text = await collectSse(response); $(id).innerHTML = `<div class="message info markdown">${renderMarkdown(text)}</div>`; }
async function collectSse(response) {
  const reader = response.body.getReader(); const decoder = new TextDecoder(); let buffer="", out="";
  while(true){ const {value,done}=await reader.read(); if(done) break; buffer += decoder.decode(value,{stream:true});
    let idx; while((idx=buffer.indexOf("\n\n"))>=0){ const block=buffer.slice(0,idx); buffer=buffer.slice(idx+2); const line=block.split("\n").find(l=>l.startsWith("data:")); if(!line) continue; const data=JSON.parse(line.slice(5)); if(data.type==="content") out += data.content; }}
  return out;
}
function typeText(id, text) {
  const el = $(id); let i = 0; el.innerHTML = "";
  const timer = setInterval(()=>{ el.innerHTML = `<div class="message info markdown">${renderMarkdown(text.slice(0, i++))}🍊</div>`; if(i > text.length) { clearInterval(timer); el.innerHTML = `<div class="message info markdown">${renderMarkdown(text)}</div>`; } }, 18);
}
function renderMarkdown(text) {
  const lines = String(text || "").replace(/\r\n/g, "\n").split("\n");
  let htmlOut = "", listOpen = false, paragraph = [];
  function flushParagraph() {
    if (paragraph.length) {
      htmlOut += `<p>${inlineMarkdown(paragraph.join(" "))}</p>`;
      paragraph = [];
    }
  }
  function closeList() { if (listOpen) { htmlOut += "</ul>"; listOpen = false; } }
  for (const raw of lines) {
    const line = raw.trim();
    if (!line) { flushParagraph(); closeList(); continue; }
    const heading = line.match(/^(#{2,3})\s+(.+)$/);
    if (heading) {
      flushParagraph(); closeList();
      const tag = heading[1].length === 2 ? "h2" : "h3";
      htmlOut += `<${tag}>${inlineMarkdown(heading[2])}</${tag}>`;
      continue;
    }
    const bullet = line.match(/^[-*]\s+(.+)$/);
    if (bullet) {
      flushParagraph();
      if (!listOpen) { htmlOut += "<ul>"; listOpen = true; }
      htmlOut += `<li>${inlineMarkdown(bullet[1])}</li>`;
      continue;
    }
    paragraph.push(line);
  }
  flushParagraph(); closeList();
  return htmlOut || "<p>暂无内容</p>";
}
function inlineMarkdown(text) {
  return escapeHtml(text).replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
}
function tableHtml(headers, rows){ return `<div class="table-wrap"><table><thead><tr>${headers.map(h=>`<th>${escapeHtml(h)}</th>`).join("")}</tr></thead><tbody>${rows.map(r=>`<tr>${r.map(c=>`<td>${escapeHtml(c)}</td>`).join("")}</tr>`).join("")}</tbody></table></div>`; }
function objectTable(rows){ if(!rows?.length) return "<p>暂无数据</p>"; const keys = Object.keys(rows[0]); return `<div class="table-wrap"><table><thead><tr>${keys.map(k=>`<th>${escapeHtml(k)}</th>`).join("")}</tr></thead><tbody>${rows.map(r=>`<tr>${keys.map(k=>`<td>${escapeHtml(r[k])}</td>`).join("")}</tr>`).join("")}</tbody></table></div>`; }

render();
"#;
