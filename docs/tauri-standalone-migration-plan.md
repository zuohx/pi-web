# Tauri Standalone 改造清单（pi-web）

## 目标

本清单对应桌面壳第二阶段优化：

> **仅优化桌面打包产物体积，不改变已经跑通的 `desktop:dev` 开发链路。**

当前首版桌面壳已经可以正常工作，但 `desktop:build` 仍然把整个 `node_modules` 带入 Tauri bundle，导致安装包体积过大。

本阶段的目标是：

- `desktop:dev` 保持现状
- `desktop:build` 改为基于 Next `standalone` 输出打包
- 尽可能去掉对完整 `node_modules` 的依赖
- 保持现有桌面运行行为不变

---

## 结论

推荐采用以下策略：

> **开发模式不改，正式打包模式切换到 Next `standalone`。**

也就是说：

- `pnpm desktop:dev` 继续使用当前 `next start` 方案
- `pnpm desktop:build` 改为使用 `.next/standalone/server.js`

这是当前项目里风险最低、收益最高的改造方式。

---

## 为什么值得做

当前正式打包配置中，`src-tauri/tauri.bundle.conf.json` 明确将以下内容作为 Tauri bundle 资源：

- `../bin`
- `../.next`
- `../public`
- `../node_modules`
- `../next.config.ts`
- `../package.json`

其中最大的问题是：

> **整个 `node_modules` 被打包进入桌面安装产物。**

这样做的原因不是 Tauri 默认行为，而是当前实现仍然依赖：

- `bin/pi-web-desktop.js`
- `next start`
- `next/dist/bin/next`

只要正式打包仍然依赖 `next start`，就很难避免把大量 Node 运行时依赖一起打包。

Next `standalone` 的作用就是：

> **输出一份最小化的服务端运行目录，仅保留构建后真正需要的运行时文件。**

---

## 改造边界

本阶段只优化桌面正式打包链路，不做以下事情：

- 不改现有 API 架构
- 不改 SSE 模式
- 不改 AgentSession / RPC 机制
- 不迁移到 Rust commands
- 不改前端请求路径
- 不让 `desktop:dev` 跟着一起切换

这次改造关注的只有一件事：

> **让正式桌面包不再依赖整包 `node_modules`。**

---

## 建议实现

### 方案原则

采用双轨方案：

1. `desktop:dev` 维持当前实现
2. `desktop:build` 使用 standalone 产物

这样做的原因：

- 开发链路已经跑通，没必要再动
- standalone 主要服务于打包与分发
- 出问题时定位范围更小

---

## 需要改动的文件

### 1. `next.config.ts`

目标：

- 增加 `output: "standalone"`

作用：

- 让 `next build` 产出 `.next/standalone`

注意点：

- 当前项目已经使用 `serverExternalPackages`
- 这会让指定依赖保持为 Node 侧运行时外部依赖
- 需要重点验证这些外部依赖是否被 standalone tracing 正确覆盖

当前高风险依赖：

- `@earendil-works/pi-coding-agent`
- `@earendil-works/pi-ai`

如果 tracing 不完整，后续可能需要补 `outputFileTracingIncludes`

---

### 2. `package.json`

目标：

- 调整 `desktop:prep`
- 明确区分 dev build 与 desktop build 所需准备步骤

建议方向：

- `build` 继续负责普通 Next 构建
- 为桌面打包新增 standalone 准备命令
- 在该准备命令中完成：
  - `next build`
  - 将 `public` 复制到 `.next/standalone/public`
  - 将 `.next/static` 复制到 `.next/standalone/.next/static`

原因：

> Next 的 standalone 默认不会自动复制 `public` 和 `.next/static`

---

### 3. `bin/pi-web-desktop.js`

目标：

- 让桌面启动器支持两种运行模式

建议行为：

1. 如果存在 `.next/standalone/server.js`
   - 优先直接运行该文件
2. 否则
   - fallback 到当前 `next start` 路径

这样做的好处：

- `desktop:dev` 无需改变
- `desktop:build` 可以自然切换到 standalone
- 回退路径清晰

建议增加：

- 更明确的启动日志
- 启动目标分支判断（standalone / next start）

---

### 4. `src-tauri/tauri.bundle.conf.json`

目标：

- 删除对整个 `node_modules` 的依赖

当前资源：

- `../bin`
- `../.next`
- `../public`
- `../node_modules`
- `../next.config.ts`
- `../package.json`

建议改为更小的资源集合，方向如下：

- `../bin`
- `../.next/standalone`
- `../public` 或 standalone 内复制后的 public
- `../.next/static` 或 standalone 内复制后的 static
- 必要时少量补充额外配置文件

原则：

> **只带 standalone 运行真正需要的内容。**

---

### 5. 可选：新增桌面专用准备脚本

建议新增一个小脚本，例如：

- `bin/prepare-standalone-desktop.js`

作用：

- 校验 `.next/standalone/server.js` 是否存在
- 复制 `public`
- 复制 `.next/static`
- 输出更清晰的构建准备结果

这样可以避免把复制逻辑塞进长命令串里。

---

## 风险点

### 风险 1：外部依赖 tracing 不完整

当前项目使用：

- `serverExternalPackages`

这意味着一些服务端依赖不会被普通 bundle 吃进去，而是继续在 Node 运行时按文件系统依赖解析。

风险表现：

- build 成功
- dev 正常
- 桌面打包后启动时报模块缺失

重点验证：

- Agent 相关 API
- 模型配置读写
- 会话读取
- RPC 启动链路

---

### 风险 2：运行时动态读取文件

如果依赖包内部有以下行为：

- 动态 `require()`
- 按相对路径读取资源文件
- 运行时探测 package 目录
- 子进程调用外部命令并依赖 cwd 附近文件

则 standalone 不一定一次就能完整覆盖。

出现这种情况时，需要：

- 补 tracing includes
- 或额外将少量资源文件加入 bundle

---

### 风险 3：桌面启动器路径假设变化

当前 `bin/pi-web-desktop.js` 的实现假设：

- 项目根目录下存在 `.next`
- 项目根目录下存在 `node_modules/next`

切到 standalone 后，这些假设不再完全成立。

所以桌面启动器必须明确区分：

- dev 路径
- build 路径

否则很容易出现：

- dev 正常
- build 包启动失败

---

## 建议实施顺序

### Step 1：只改 Next 构建输出

- 在 `next.config.ts` 加 `output: "standalone"`
- 本地执行 `next build`
- 确认 `.next/standalone/server.js` 生成成功

目标：

- 先证明 standalone 产物能生成

---

### Step 2：补齐静态资源复制

- 复制 `public`
- 复制 `.next/static`

目标：

- 让 standalone 目录具备完整运行资源

---

### Step 3：改桌面启动器

- `bin/pi-web-desktop.js` 优先跑 standalone
- 保留 dev fallback

目标：

- 不影响当前开发链路

---

### Step 4：缩小 Tauri bundle 资源

- 更新 `tauri.bundle.conf.json`
- 移除整个 `node_modules`

目标：

- 让正式桌面包真正变小

---

### Step 5：验证正式打包

至少验证以下场景：

1. `pnpm desktop:build` 成功
2. 安装包 / exe 能启动
3. 首屏能打开
4. API 可用
5. SSE 可用
6. Agent 请求可用
7. 关闭窗口后后台服务正确退出

---

## 验证清单

完成改造后，必须验证：

- standalone 目录存在
- `server.js` 可被 Node 启动
- `public` 资源正常加载
- `.next/static` 资源正常加载
- 桌面窗口打开成功
- 会话列表正常读取
- 打开旧会话正常
- 发送消息正常
- SSE 流正常
- 模型配置读写正常
- 应用关闭时后台 Node 进程被清理

---

## 推荐结论

推荐继续推进 standalone 改造，但方式必须是：

> **仅切正式打包链路，不动已经稳定的开发链路。**

换句话说：

- 现在不应该把 `desktop:dev` 一起重写
- 应该把风险压缩在 `desktop:build` 上

这是当前项目里最稳妥的升级路径。

---

## 最终判断

如果问题是：

> **“这个项目值不值得从当前整包 node_modules，升级到 standalone 打包？”**

答案是：

> **值得。**

如果问题是：

> **“应该一次性全切，还是只改正式打包链路？”**

答案是：

> **只改正式打包链路。**

这条路线最符合当前项目状态，也最符合风险控制原则。
