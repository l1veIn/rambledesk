# Web 部署（Cloudflare Pages + rambledesk.com）

每次往 `main` 推送且 `web/**` 有改动时，CI（`.github/workflows/web-deploy.yml`）自动构建网站并发布到 Cloudflare Pages。

## 一次性设置（只需做一次）

### 1. 重新登录 wrangler（本地 token 已过期）

```bash
npx wrangler login
```

浏览器授权完成后验证：

```bash
npx wrangler whoami
```

### 2. 创建 Pages 项目

```bash
npx wrangler pages project create rambledesk-web \
  --production-branch main
```

### 3. 创建 API Token（供 CI 使用）

Cloudflare Dashboard → **My Profile → API Tokens → Create Token** →
选择 **Cloudflare Pages — Edit** 模板 → 账户选主账户 → 创建。

### 4. 在 GitHub 仓库配置 Secrets

仓库 **Settings → Secrets and variables → Actions → New repository secret**：

| Secret | 值 |
|---|---|
| `CLOUDFLARE_API_TOKEN` | 上一步创建的 token |
| `CLOUDFLARE_ACCOUNT_ID` | Cloudflare 账户 ID（Dashboard 首页右侧/Profile 页面） |

### 5. 绑定域名 rambledesk.com

- 确认 `rambledesk.com` 的 DNS 已托管到 Cloudflare（在 Cloudflare 添加站点后，把域名商处的 NS 改成 Cloudflare 提供的两个，等 NS 生效）。
- Cloudflare Dashboard → **Workers & Pages → rambledesk-web → Custom domains → Add** → 输入 `rambledesk.com`（可用 `www.` 变体各加一个）。
- 若域名商不是 Cloudflare，也可以直接在 Cloudflare 加 `CNAME` 记录指向 Pages 项目的 `*.pages.dev` 地址（Pages → 项目 → 自定义域 → CNAME setup 提示）。

## 日常流程

什么都不用做：`git push` 到 `main` 且改了 `web/` → CI 自动构建并发布。

本地预览构建产物：

```bash
cd web && npm run build && npx wrangler pages deploy dist --project-name rambledesk-web
```
