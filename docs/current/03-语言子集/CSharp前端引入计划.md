# C# 教学子集前端引入计划（2026-09-11，v3 定稿）

> **决策背景**：SharpTutor（WPF/.NET 的 C# 教学 IDE）是本仓库前端切割后的**第一个外部消费者**，
> 已完成 C/C++ 引擎集成对接（capi 签名评审定稿、serve 主路径、15 用例冒烟集，见
> `CAPI评审回复与实现状态.md`）。在此之上，其"多语言对比教学"要从静态并排面板升级为
> **可执行白箱对照**，需要原生 C# 引擎：算法可视化自动检测 + 极细颗粒度纠正。
> 其自研 Roslyn 转译路线（C# 降解为 C 再调 Vitro）已实证**文本层降解撞语义错位**——报错
> 定位在生成代码行号、误区模式表无法映射 C# 引用语义——从反面验证了原生白箱前端的必要性。
>
> **立项三条件齐备**：锚定客户（13 章课程代码 = 天然验收用例集）、真实需求（对方明确的
> 消费缺口）、替代路线已排除（转译器降为参考实现，见 §8）。
>
> **执行时机**：本仓库 `结构重构与C23锚定决议.md` 全批次交付后启动（时序见 §7）。
> 本计划 v3 已吸收 SharpTutor 消费侧评审（2026-09-11），吸收记录见 §11。

## 1. 定位边界声明

**定位扩展**：教学 C/C++ 子集参考执行引擎（白箱）——主轴不变，新增 **C# 教学子集作为
第二前端**，由 SharpTutor 诉求锚定。原"后端语言锁定 C/C++"口径（定位主计划 §0）随之
扩展，**待在 `后端定位与白箱计划.md` 同步声明**（待办，见 §11）。

**不换（行为资产，与重构计划同一张清单）**：
- 编译管线五段结构（Lexer → Parser → TypeChecker → CodeGen → VM）
- 字节码格式与 671 个 Shadow golden（C# 侧只允许按既有规则**追加** opcode，本计划追加 3 个）
- 出口协议（capi ABI / serve 协议 / StepPayload schema——只增不改语义）
- VM 1MB 线性内存模型与教学检测语义（UAF/Double-Free/泄漏报告）

**换**：
- 语言分派：`is_cpp_mode` 布尔穿透 → `enum SourceLang { C, Cpp, CSharp }` 单源化
  （现状唯一检测点 `compile_pipeline.rs:603`，`Session`/`CompileState` 无语言字段，9 处
  `"main.c"` 兜底字符串按扩展名生成）
- 新增 `vitro_csharp_frontend` crate（依赖只到 `vitro_shared`/`vitro_ast`）
- 新增 3 个 opcode（`TryBegin`/`TryEnd`/`Throw`，C++ 扩展以来首次——成本清单：
  `opcode.rs` 枚举 + `executor/mod.rs` 分发 + JIT 白名单 + 跳转重定位核对）

**明确不做**：
- **Roslyn 进引擎**：wasm 出口（Roslyn 进不了 wasm32，白箱形态会死）+ 诊断主权
  （E 码/中文修复建议/子集裁决是引擎卖点，CSxxxx 标准诊断替代不了）。Roslyn 固定在
  防线侧（golden 生成）与消费侧（SharpTutor 编辑器波浪线/补全），与引擎互补。
- 动态语言 variant VM / Python 前端（前序评估已裁：赛道有霸主、无锚定客户）
- ORC/所有权模型降解（C# 心智模型是"引用"，所有权是额外概念负担）

## 2. 核心设计裁决

| # | 裁决 | 内容 |
|---|------|------|
| D1 | 语言标识单源化 | `SourceLang` enum 替代布尔穿透；typeck/codegen 现由 AST 节点自感知（C++ 先例），C# 沿用 |
| D2 | 对象模型 | 引用类型 = 堆分配（`CallHost(MALLOC)`，C++ `new` 先例）+ 引用即 u32 指针 + **ARC 确定性回收**（见 §3） |
| D3 | 子集边界 | 以 SharpTutor ch01–04 课程裁剪（§5），第一版明确不做 LINQ/委托/属性/异常以外的高级特性 |
| D4 | 错误码 | **E5xxx 新段**（E4xxx 已被 C++ 占用，E4001~E4031）；`ErrorInfo` 补"适用语言"维度字段（SharpTutor 既有诉求，且 E4001~E4031 目前在 catalog 无条目——存量缺口一并补） |
| D5 | 诊断管线 | **机制复用、数据表分语言**：TraceAnalyzer/误区模式/概念图/学习路径的机制（滑窗计数、图激活、路径组装）语言无关，C# 换错误码表 + 模式表 + 第二张概念图 |
| D6 | StepCollector 参数化 | C 硬编码两处（`collect_pointer_snapshots` 的线性内存假设、`infer_semantic_label` 的 C 库函数文本启发）按 `SourceLang` 分派；C# 模式下指针快照语义变为**引用快照**（schema 只增） |

## 3. ARC：引用类型生命周期（替代 GC）

**裁决**：GC 不是教学内容，降解为 **ARC 确定性模型**——引用类型堆分配，region 元数据挂
`refcount: u32`，赋值/传参/返回/作用域退出/字段与集合元素覆盖时插 release/retain 对，
计数归零立即释放。

**实现要点**（全部有仓库先例）：
- Retain/Release 走 `CallHost(RETAIN/RELEASE)` 两个新 host id，**零新 opcode**（参数走
  操作数栈传 u32 地址，既有 host func 约定）；
- 插桩点由 C# 前端按静态类型判定，工程性质 = `vitro_codegen/cpp/raii.rs` 的 LIFO 析构
  插桩同族；
- `refcount` 随 `MemorySnapshot.regions` 自动序列化——**时间旅行免费安全**；
- 与真实 .NET 对象头的计数位置差异不可观测（子集无 unsafe、无 `sizeof(class)`）。

**为什么成立**：
1. 行为对齐问题被子集边界消解——子集不支持 `GC.Collect`/`WeakReference`/Finalizer/
   `IDisposable` 模式，ARC 与 tracing GC 的差异在 **stdout 通道不可观测**，shadow 防线
   照常（golden 由 dotnet 生成，逐字节一致）；
2. "不回收"方案有真实撞墙风险：1MB 堆区约 900KB，密集 new 循环 3~5 万次即 trap，
   会制造假教学错误（真实 .NET 不会）；
3. 白箱增值：计数变化实时可见 = "值类型 vs 引用类型赋值语义"（C# 教学头号难点）的
   可视化，.NET 真实 GC 反而给不了。

**循环引用**：ARC 下永不释放 → 泄漏报告抓出 + E5xxx 教学卡（"这是 ARC 学习点不是错误，
真实 .NET 的 GC 会回收"）。双向链表类用例配专门话术。

**诚实记录（进 C# 子集 spec）**："引用类型生命周期采用 ARC 确定性模型降解；与 .NET
tracing GC 的差异在子集输出通道不可观测；内存面板展示 ARC 模型；循环引用为 ARC 特有
教学点。"ARC 的机制层（region 表 + refcount + 确定性释放）未来可反哺 C 侧别名教学。

## 4. 异常设计（核心章节，教材高频特性，不裁）

《C# 本质论》自第 5 章起随手使用 `try/catch/finally/throw`，防御式编程（`throw new
ArgumentException`）本身是教学内容。且**栈展开逐帧动画是 C 侧结构上做不出来的独家白箱
画面**。

### 4.1 字节码层：显式 handler 栈（不用异常表）

真实 CLR 用异常表 + zero-cost 展开，教学子集不用——范围查询复杂、白箱不可见。新增
3 个 opcode：

```
TryBegin <handler_idx>   ; 压入 handler frame
TryEnd                   ; 弹出 handler frame（正常离开 try 范围）
Throw                    ; 抛出：栈顶是异常对象引用
```

VM 新增执行状态：
- **handler 栈**：`Vec<HandlerFrame { catch_ip, frame_idx, stack_depth, mem_stack_top }>`。
  `Throw` 时恢复三项并跳 `catch_ip`，try 块内中间计算值自然作废；
- **当前异常寄存器**：异常对象引用。catch 变量 `catch (Exception e)` 即存入局部变量
  （走 ARC +1）。

catch 匹配按类型链：内置异常族（约 7 个）硬编码继承链，用户自定义异常类查声明链。
多 catch 自上而下、`e.Message` 读取均为普通字节码。

### 4.2 跨函数展开：UNWINDING 状态机 + 逐帧 step

`Throw` 时当前帧无 handler → 进入 **UNWINDING**：逐帧弹出 `call_stack`，每帧批量
release 局部引用（见 4.6）、执行该帧未完成的 finally 块，向上一帧。
**每弹一帧 / 执行一个 finally 块 = 一个 VM step**（行为契约，见 §6-B）。由此直接获得：
时间旅行进度条上展开是平滑可 seek 区间、`call_stack` 逐帧缩短驱动栈展开动画、实现是
普通指令流。

finally 的正常路径语义（return/break/continue 穿过 try 块时执行）由 codegen 在控制流
出口插桩（`raii.rs` 同族先例）。finally 内抛新异常覆盖旧异常按 .NET 自然语义。

### 4.3 内置异常映射：现有 trap 转 throw

| 教材场景 | C 侧行为 | C# 侧行为 |
|---|---|---|
| 整数除零 | trap + 教学诊断 | `DivideByZeroException` |
| 数组越界 | `TrapBounds` trap | `IndexOutOfRangeException` |
| 空指针解引用 | NULL trap | `NullReferenceException` |
| `int.Parse("abc")` | atoi 静默 | `FormatException` |
| `throw new ...` | — | 显式 Throw |

实现落点在 **codegen 层**（C# 模式的检查生成 Throw 序列而非 trap 指令），VM trap 语义
本身不动——C 侧 636 golden 与检测语义零影响。

**未捕获 = trap，含过渡语义**：传播出 `main` → 程序终止，trap 消息 .NET 风格
（`Unhandled Exception: System.DivideByZeroException: ...`）。**CS3a 阶段（无跨帧展开）
Throw 遇当前帧无 handler 时直接 trap**，消息格式与 `error_code` 与最终一致——
CS3b 只把"立即死"改为"逐帧展开后死"，消费端判分契约（`run_json` status 三态 +
`trap.error_code`）两段之间零变化。

**shadow 口径**：判分用例一律 catch 后输出（SharpTutor fixtures 编写规范）；未捕获
异常的终止输出中**栈轨迹部分与 dotnet 不逐字节一致**（简化轨迹），golden 比对只对
message 前缀。进诚实记录。

### 4.4 `throw;` 与 `throw e;`：轨迹保留差异（教材考点，随 CS3a）

`throw;` 保留原始栈轨迹 vs `throw e;` 重置——《本质论》明确警告后者的经典陷阱题。
实现：异常对象挂"原始抛点"字段，`throw;` 不改写、`throw e;` 改写为当前点，未捕获
终态消息展示原始抛点行号。增量 = 一个字段 + Throw 双形态判别。`throw;` 帧内形态随
CS3a（当前异常寄存器与嵌套 try 的外层 handler 在 CS3a 即工作），CS3b 自然获得跨帧
重抛。

### 4.5 × 时间旅行：最深技术点

handler 栈进 `VMSnapshot`（新字段）。**UNWINDING 中间态也是可快照状态**：须记录
"当前异常寄存器 + 剩余待展开帧 + 目标 handler"，否则 seek 落在展开区间中间恢复出
撕裂状态。CS3b 显式设计 + 快照往返测试（展开中间态 snapshot → restore → 继续展开
到同一终态）。异常对象在堆上由 `MemorySnapshot` 自然随行。

### 4.6 × ARC：展开清理复用 dtor 列表先例

展开弹帧时被弹帧的引用类型局部变量逐个 release——落点为函数表已有的"帧退出清理列表"
（C++ RAII 为 scope exit/析构所建，同一列表、动作换 release）。异常对象由当前异常
寄存器持有（+1），catch 结束 -1。零新机制。

### 4.7 × JIT

含 `TryBegin` 的函数**排除出 trace 编译**（与"含断点的循环排除"同一模式），行为契约化
写明。教学程序性能无感。

## 5. 子集口径 v1（待 SharpTutor 红线清单回填）

**支持**：值类型（struct 按值语义，栈分配）/ class（字段/方法/构造/this/静态成员，照搬
C++ 多 Pass：类布局注册 → this 注入 → 方法降级 mangled C 函数）/ 继承 + 虚函数
（虚表 + `CallPtr`）/ 数组 / foreach（`RangeFor` 先例）/ `List<T>`、`Dictionary<K,V>`
（Phase 41 模式：JSON 接口声明为唯一真相来源，`vitro_cpp_frontend/builtin_layout.rs`
加载器换数据文件）/ try-catch-finally-throw / 7 个内置异常 / `e.Message` / 自定义
异常类 / `throw;` / `Console.WriteLine/ReadLine`（host func 映射）。

**裁剪**：`when` 过滤器、checked/`OverflowException`、async、委托/事件、属性语法糖、
LINQ、自定义泛型、`IDisposable`/using 模式、Finalizer、`WeakReference`、`GC.Collect`、
`unsafe`、`decimal`、多文件工程、运算符重载。

**语料画像（2026-09-12 实测，SharpTutor `CourseData`，贡献方确认全量贡献）**：
13 章 521 个 `.cs`，每课自带 `solution.cs` / `exercise.cs` / `expected.txt`
（dotnet golden 已存在）/ `fixtures.json`（**多 fixture**：同一程序多组
stdin→expected，如 alg01 三组判分）。章节→批次映射：ch01–04（78 文件）= CS1
验收面；ch05–06（63）= CS2；ch07（45）= CS4；ch08 LINQ / ch10 Assembly /
ch13 WPF = 裁剪区；ch12_AlgoBank（153，多 fixture 算法库）= CS6 同族。

**ch01–04 特性画像（红线清单的事实底座）**：顶层语句 **100%**（全语料亦然——
无 Main，入口合成是 CS1 硬需求）；`int.Parse/TryParse` 24%、`Console.ReadLine`
23%、表达式体方法 17%、`var` 11%、try/catch 8%（这 7 个用例归 CS3a）、插值 6%、
class 5%、foreach 3%、List 1%、继承 0%（ch05 起）。§5 草案据此修订：
+顶层语句（入口合成）、+表达式体方法、+`var`、**+字符串插值（基础形态：
变量/表达式插值，随 CS1）**——6% 用量看似低，但它是 ch02 的教学主特性、C# 的
签名级语法，教材自第 2 章起随手使用；格式化说明符（`:F2` 等文化感知格式化）
后置 CS4 之后单独裁。`Convert.` 待红线裁决。

**诊断管线 C# 数据表**（CS5）：误区模式首批 = 空 catch 吞异常、`catch (Exception)` 过宽、
finally 里 return、`==` vs `Equals`、可变列表别名、循环边界 off-by-one 的 C# 表述
（`<=` 配 `arr.Length`）、`List<T>` 容量与 Count 混用、foreach 中修改集合、字符串不可变
性误解（`s.Replace` 非原地）。概念图加异常域节点（栈展开/异常层次/try-catch-finally
顺序）。

## 6. 协议增量（schema v0.1 预留位 + 行为契约）

**A 档：StepPayload 增量字段（四项，随 v0.1 冻结以预留位写入
`spec/STEP_PAYLOAD_SCHEMA_V0_1.md`，CS3b 激活）**：
1. `handler_depth: int`——"当前受几层 try 保护"直读；
2. `unwinding: bool` + `unwind_frames_left: int`——展开态显式标记 + 剩余帧数，
   展开动画驱动字段；
3. `current_exception: {type_name, message, addr, origin_line} | null`——当前异常
   寄存器可见；`addr`（u32 堆地址）联动内存面板 region 高亮（ARC 教学闭环）；
   `origin_line`（原始抛点行号，2026-09-12 评审补充）——`throw;` 保留 /
   `throw e;` 改写（§4.4），知识卡片"原始抛点在第 X 行"结构化直读本字段，
   不解析 trap message 文本。

**B 档：词汇契约**：
- 行为契约：**"UNWINDING 每帧一 step，展开不可被合并成单步"**（与"JIT 断点完整性"
  同列行为契约清单，防未来性能优化毁掉展开动画）——**已落代码**：
  `unified/contracts.rs::BEHAVIOR_CONTRACTS` 条目 `unwinding_step_granularity` +
  可执行判据 `check_unwinding_granularity`（schema §9.3）；
- 行为契约：**含 `TryBegin` 的函数排除 JIT trace**（同表条目 `try_excludes_jit`）；
- `semantic_label` 词汇表补异常域条目（"抛出 IndexOutOfRangeException"/"展开弹出
  Main 帧"/"执行 finally 块"/"捕获 DivideByZeroException"）并进 schema 附录，
  **词汇即契约，消费端 UI 直读不做推断**。
  ——此项同时是重构计划 R3"教学标注双来源"的收口方案：`semantic_label` 从自由文本
  启发升级为受控词汇表枚举，`collector.rs` 的启发逻辑降为词汇表映射函数。
  **2026-09-12 落地**：schema 附录 B + `unified/vocabulary.rs` 单源 + serve
  `semantic_labels` 出口 + 词汇闭包防线（SharpTutor S4 §6 异常域首批 4 条以
  `reserved` 状态预先登记，激活即"契约兑现"而非"新增契约"）。

**C 档：schema 签字回放场景第四组**（前三组：防抖编译流/判分流/单步+seek+内存查询
交错流）：**异常交错流**——throw → 逐帧展开 → catch → 再 throw → finally 内 return，
l07 形态真实课程代码。

## 7. 批次计划与触发时序

**时序**：Phase 1（capi 第一批 + serve + Issue A/B，schema v0.1 冻结**含预留位**）
与重构计划剩余批次（E1 → R2 → E2 → R3 → E3 → R4；R1 已完成）→ **重构计划全批次交付
后启动 CS0**。

**触发/验收前提**：重构计划验收线保持（Shadow 0 非预期差异、cargo test 全绿、
clippy 零警告、serve 冒烟通过）；R2 已完成（CS0 的 `SourceLang` enum 在会话收口后
的代码基线上独立做，不再搭 R2 车）。

**整备计划时序同步（2026-09-12 v1.1 吸收）**：[`三语化整备审计计划.md`](../../07-质量与裁定
的止血批次插入本批次序列——**S0（断言强度审计）与 S1（教学内容防线）于 CS0 前/并行**
（第一批 C# 用例进防线时口径须已换代）；**S2（槽位 + 统一模式资源生命周期重构）、
S2.5（宿主资源域收口）、S3（C++ 收口）为 CS2 与 CS3 的双重硬前置**——CS2 的 ARC 插桩
密度压在槽位病灶上，CS3b 的异常交错回放直接落在统一模式子系统上（10591ad 泄漏事故的
两洞之一仍在，见整备计划 §5.0）。未完成不开对应批次。

**统一路线图映射（2026-09-12 v1.1，排期权威）**：S 系列批次表已并入
[`统一整备路线图.md`](../../07-质量与裁定（U0~U7），本计划的硬门禁以路线图
为准——**U0（观测+口径换代+C ABI 契约止血）于 CS0 前；U1（教学内容防线+P0 静默错值
收口）与 CS0/CS1 并行；U2（资源生命周期重构）+ U3（槽位/C++ 寄生/模板手术）为 CS2/CS3
双重硬前置**。S→U 映射：S0 + S2.5 部分→U0；S1→U1；S2-A→U3、S2-B→U2；S3→U3；S4→U6。

**中止与改判判据（2026-09-12 v1.2 吸收，[`核心资产重构裁定.md`](../../07-质量与裁定 §7.3/§Q6）**：
① **R5 判据直接约束 CS 批次**——C# 引入（CS2/CS3）后 3 个月内，因"单遍 codegen + 固定槽位/
多 pass 收敛"结构产生 ≥3 起合法性缺陷（误拒合法代码或静默错值）→ 单遍 codegen 结构本身进
重判（不止是点修）；② 中止条件——任何阶段发现 P0 级新事故（GB 级资源事故/静默错值扩散到
核心语义）→ 立即冻结该阶段按事故归档制度留痕；③ CS 批次缺陷须按"缺设计 vs 缺补丁"归因
入台账（R5 的判定输入），嵌入 CS2 起的批次验收。

| 批次 | 内容 | 关键验收 |
|------|------|---------|
| **CS0** | `SourceLang` enum + 语言分派单源化 + `"main.c"` 兜底 9 处按扩展名 | 全防线绿 |
| **CS1** | `vitro_csharp_frontend` 骨架：lexer/parser（§5 语法面）/薄 typeck/降级 codegen，Hello World + 值类型 | C# shadow 小集（SharpTutor dotnet golden）；**验收面 = ch01–04 课程代码**（对方提供） |
| **CS2** | class/方法/构造/this/静态 + 继承虚函数 + ARC（§3 全部） | **预览版交付点**：白箱跑算法 + ARC 别名可视化，SharpTutor 透视模式试用（de-risk CS3 投入） |
| **CS3a** | 3 opcode + handler 栈 + finally 插桩 + 内置异常映射 + `throw;`（含 §4.4）+ handler 栈快照 | 帧内 try/catch/finally/throw 全用例；除零/越界/空引用可捕获；无 handler 即 trap（契约稳定） |
| **CS3b** | UNWINDING 状态机 + 跨帧展开 + 展开清理（§4.6）+ 展开中间态快照/seek 往返 | 第四组回放场景；栈展开动画字段（A 档）激活；展开中间态往返测试 |
| **CS4** | 数组/foreach/List/Dictionary（越界→可捕获异常接 CS3a 通路） | 容器用例 |
| **CS5** | E5xxx + catalog 语言字段 + semantic_label 词汇表化（**R3 收口项落地**）+ TraceAnalyzer 异常类目 + C# 误区/概念表 + **G9 算法验证**（`validation.rs` 语言中立层，41 模板元数据为基础） | 认知管线 C/C# 双语出山（首次接线外部消费者） |
| **CS6** | 算法标注审计平移（`AlgorithmContext` trait 解耦良好，41 个 infer 函数逐个审 C 文本模式）+ LeetCode C# 批次 | **differential 双语言标注一致性**（同题 C/C# 解法的算法名/phase/置信度一致，用例集 SharpTutor 提供） |

量级参考：CS1 对标 E2（~2.5k 含测试）；CS3（a+b 合计）约 2~3k 含测试，快照状态机
是大头。

## 8. 双方分工与对等承诺（SharpTutor 评审确认）

**上游（本仓库）**：引擎、管线、协议、防线。

**SharpTutor**：
1. **C# 语料全量贡献**（2026-09-12 确认，本地镜像 `D:\code\SharpTutor`）：
   CourseData 13 章 521 个 `.cs` + 76 个 `expected.txt` golden + `fixtures.json`
   多 fixture 清单。**shadow 驱动与 golden 生成管线归本仓库**（2026-09-12 修订，
   原"golden 由其生成"调整为"语料由其贡献、管线/门禁归引擎仓库"——防线自服务
   与复现性要求 dotnet 版本钉死、用例改动自刷新 golden；跨仓库供货会复刻
   G10/G12 类漂移。dotnet 之于 C# shadow = clang 之于 C shadow，都是外部参考
   实现）；
2. **转译器再定位**：Roslyn 转译器从"替代路线"降为 **CS1/CS2 期间参考实现**（转译
   产物 = 子集用法活文档）+ 原生前端未覆盖特性的快速通道；CS0 前作为需求验证工具
   产出透视模式使用数据；
3. **CS1 验收面**：ch01–04 课程代码即子集用法测试集；
4. **红线清单 C# 版**：子集口径确定后 48 小时内交付（同 C 侧机制），含"判分用例一律
   catch"规范。

## 9. 教学增值（组合画面承诺样例）

学生写的冒泡排序第 3 轮把交换条件写反 → `algorithm_step` 识别"冒泡排序，phase=交换"
但结果序列错误 → TraceAnalyzer 误区模式命中"交换方向反了" → 知识卡"比较的是
arr[j] > arr[j+1]，你想升序还是降序？" → seek 回交换发生的那一步看变量。
——这条链每一环的协议字段已存在或已列入 §6-A 档，C# 立项使它对 C# 代码同样成立。
另：**栈展开逐帧动画**（异常从第 3 层抛出 → 逐帧弹出 → finally → catch 接住）为
C# 白箱独有画面。

## 10. 诚实记录预设（进未来 CS_SUBSET_SPEC）

1. ARC 降解差异（§3 引文）；
2. 未捕获异常终止输出的轨迹简化（§4.3）；
3. `throw;`/`throw e;` 轨迹保留语义按"原始抛点字段"口径实现，与 .NET 完整栈轨迹
   不同（简化轨迹 + 原始抛点行号）；
4. `refcount` 位于 region 侧表而非对象内存（对子集不可观测，记录实现口径）；
5. C# 基础数值类型（`int` 32 位 / `long` 64 位 / `double` IEEE）与引擎现有模型对齐，
   无精度差异；`decimal` 不支持（§5 裁剪）。

## 11. 状态记录

- **2026-09-11 v3 定稿**：吸收 SharpTutor 消费侧评审——三点决策答复（`throw;` 保留并
  连带轨迹考点进 CS3a；输出契约确认判分器依赖 `run_json` 三态 + `error_code`；
  CS3 拆 a/b 两段独立验收）；A 档三字段 + `addr` 微增，以 v0.1 预留位随 Phase 1
  冻结写入（待办：编辑 `spec/STEP_PAYLOAD_SCHEMA_V0_1.md`）；B 档词汇表契约确认为
  R3 收口方案；第四组回放场景进签字材料；G9 排期确认（CS5）；ARC 裁决（替代早期
  "计数只做可视化"方案）；Roslyn 定位（防线 golden + 编辑器诊断，不进引擎）。
- **前置待办（2026-09-12 全部清账）**：① ✅ 定位扩展已同步进
  `后端定位与白箱计划.md` §2；② ✅ schema v0.1 预留位已写入
  `spec/STEP_PAYLOAD_SCHEMA_V0_1.md` §7.x（四字段 + 消费方容忍要求）；③ ✅
  重构计划全批次交付完毕（R1→E1→R2→E2→R3→E3→R4，最终 CI 全绿 968a408）。
- **语料调研完成（2026-09-12）**：CourseData 全量摸底（13 章 521 文件 / 76
  golden / 多 fixture 清单），章节→批次映射与 ch01–04 特性画像已录入 §5；
  **新增 CS1 硬需求**：顶层语句入口合成（语料 100% 无 Main，§5 草案未覆盖）。
- **评审吸收（2026-09-12）**：① §6-A `current_exception` 补 `origin_line`
  结构化字段位（原始抛点行号，`throw;`/`throw e;` 轨迹考点的知识卡片直读，
  不解析 trap 文本）；② 字符串插值基础形态（变量/表达式）纳入 CS1，格式化
  说明符后置 CS4 后裁。
- **schema v0.1 签字回放完成（2026-09-12）**：S1–S5 对端材料采纳回放
  **61/61 PASS**（驱动 `scripts/replay/replay_s1_s5.go`，D5 迁移后），暴露并修复五个引擎
  缺陷（越窗 seek 负下标无限分配/锚点裁剪/重放区间排他/断点双层暂停/入口步
  误标递归）。**v0.1 冻结的最后一道检查已过**，S4 激活契约表就位（CS3b 交付后
  以同序列回放 §5 断言）。**CS0 可以开工**。
- **CS0~CS6 待启动**：CS0 不依赖外部（纯内部重构）；CS1 起语料经红线清单裁定
  后接入，`shadow_verify_csharp.py` 最小版随 CS1 落地（dotnet 版本钉死，
  缺失即 fail-fast，与 clang 缺失同口径）。
- **v0.2 激活轨道落地（2026-09-12，响应 SharpTutor 需求清单 B2）**：§6-A/B 的共识
  已固化为**上游测试位**而非文档承诺：
  - schema 新增 **§9「v0.2 激活轨道」**（激活清单五条 + 字段台账 + 行为契约表）与
    **附录 B「`semantic_label` 受控词汇表」**（C 域 10 条 active + 异常域 4 条 reserved）；
    **v0.1 正式冻结**（S1–S5 61/61）；
  - 代码单源 `native/src/unified/contracts.rs`（预留位字段名 / 激活清单 / 字段台账 /
    行为契约）与 `native/src/unified/vocabulary.rs`（词汇表 + `classify()`）；
    出口 serve `contracts` / `semantic_labels`，`capabilities` 增 `schema` 与
    `behavior_contracts`；
  - **激活 tripwire**：预留位字段一出现在任何 payload（含嵌套）即让冻结测试失败并打印
    激活清单——CS3b 落地时不可能"忘记重跑 C1–C3/S1–S5 或忘记更新 §7 校验表"；
  - **`UNWINDING 不得合并单步`**（§6-B 行为契约）有了可执行判据
    `contracts::check_unwinding_granularity`（下降幅度 ≤ 1；`finally` 步可持平），
    S4 §5 A2/A5 回放将直接复用；
  - **CS5 的 R3 收口项已提前半步**：`collector.rs` 的启发逻辑现已受词汇闭包测试约束
    （产出未登记标签即失败），CS5 只需把启发降为"词汇表映射函数"。
- **三语化整备吸收（2026-09-12，整备计划 v1.1）**：[`三语化整备审计计划.md`](../../07-质量与裁定
  裁定——10591ad 泄漏事故（63.6GB / 页面文件 33.9GB）及复发的根因实锤（seek 正向重放
  无界收集，与负下标同管道两洞堵一），**统一模式子系统升级为重构区**；本计划 §7 已同步
  硬前置（S0/S1 → CS0；S2/S2.5/S3 → CS2 与 CS3）。本计划的架构裁决（ARC / 异常 /
  词汇契约）不变。
