# MoonBit 迁移 · `vitro_codegen` 模块勘察报告

> **日期**：2026-09-18 ｜ **性质**：只读勘察（迁移前置证据采集），**未执行任何仓库内文件改动、未执行任何 git 命令**
> **勘察对象**：`native/crates/vitro_codegen/` 全部源码（28 个 `.rs` / 6,237 行）
> **上游依据**：[`MoonBit迁移方案评估报告20260918.md`](MoonBit迁移方案评估报告20260918.md) §2 事实核查 / §7 四道证伪门 / §8 假设清单 A1–A9
> **真值口径**：全局数字一律引 `reports/facts.json`（`generated_at` = 2026-09-18T01:36:48+08:00，`git.rev` = `4b57191`）并标 `as_of`；本报告内的规模/计数为**本次实测**，属 as-of 快照，冻结不回填
> **勘察方法**：亲读 + 4 路并行只读子勘察 + 只读探针（`native/target/{release,debug}/vitro_cli.exe` 从 stdin 读源码；clang++ 22.1.4 取 golden）。探针产物仅写系统 `%TEMP%`（`vitro_probe_det/`、`vitro_moon_probe/scratch/`），**仓库内零改动**
> **探针诚实性**：附录 A 的缺陷 ★A / ★B 为**本次新发现**，在 release（2026-09-15）与 debug（2026-09-18）两个既有二进制上复现；**未重新构建引擎**，"HEAD 源码是否仍如此"标**待证**（验证法见附录 B）
> **纪律**：`docs/archive/` 全程未读未引用；不确定项一律标"待证"并给验证方法；缺陷按防线哲学第 0 条如实记录，不粉饰

---

## ① 模块概览与规模（实测）

### 1.1 规模

| 项 | 值 | 数法 |
|---|---|---|
| `.rs` 文件数 | **28** | `Get-ChildItem -Recurse -File -Filter *.rs`（crate 目录无 target/） |
| 总行数 | **6,237** | 逐文件 `(Get-Content).Count` 求和；含 `tests.rs` 137 行 |
| 最大文件 | `lib.rs` **950** | 其后：`expr/assign.rs` 659、`stmt/var_decl.rs` 637、`expr.rs` 559、`expr/call.rs` 450 |
| crate 身份 | `vitro_codegen` v0.1.0，edition 2021 | `native/crates/vitro_codegen/Cargo.toml:1-4` |
| 依赖 | **4 个 path 依赖**：`vitro_shared` / `vitro_ast` / `vitro_runtime` / `vitro_cpp_frontend` | `Cargo.toml:7-10`（无 dev-dependencies / features / build.rs） |

子模块分层（行数）：`lib.rs` 950（入口 + 全局/函数注册 + 槽位原语）/ `expr/` 11 文件 2,494 / `stmt/` 6 文件 1,103 / `cpp/` 4 文件 292 / `init.rs` 237 / `func.rs` 94 / `expr.rs` 559 / `tests.rs` 137。

### 1.2 关键类型（实测）

- **`BytecodeGen`：47 个字段的可变生成器状态机**（`lib.rs:37-106`）。字段族：
  - 指令流与错误（`code` / `errors`）；
  - 函数表与索引（`func_table` / `func_index` / `next_func_idx`）；
  - **四张平行符号表**（`local_indices`+`local_types`、`static_local_indices`+`static_local_types`、`global_indices`+`global_types`，均 `HashMap<String,_>`，`lib.rs:46-51`）；
  - **六个槽位字段**（`temp_slot0..3`、`temp_slot_64`、`init_base_slot`，`lib.rs:63-76`）+ **两个槽位辅助状态**（`assign_nest_depth` / `assign_addr_slots`，`lib.rs:79-81`）；
  - 全局区游标 `next_global_offset`、两组常量池、`symbols`+`sym_index`、类型布局表、5 组跳转补丁栈。
- **`CompileOutput`：13 字段**（`lib.rs:929-947`），codegen 对外的全部契约；**其中 5 个字段不进序列化产物**（见 §⑧ 8.2）。
- **分发 trait**：`ExprGen`（`expr.rs:29-45`，9 个方法）+ `StmtGen`（`stmt/mod.rs:14-17`，2 个方法）——"单体 47 字段结构 + 分文件扩展方法"形态，非真正组件化。
- **门面**：`generate()`（`lib.rs:210-575`）串起 Pass 1 全局注册 → Pass 2 函数元数据 → Pass 3 函数体 → 入口 wrapper（`lib.rs:538-550`）；**无独立 IR，边遍历 AST 边发射**。

### 1.3 发射面实测事实

- opcode 全集 **132 条**（`opcode.rs:24-156`；编号 0..137，空号 44–49）。**codegen 只引用其中 127 个**；从不发射的 5 个：`And` / `Or` / `Memcpy` / `Strlen` / `TrapBoundsVla`（前 4 个仅 VM/JIT 侧可执行；`TrapBoundsVla` 全仓无发射点，VLA 边界实际走 `TrapBounds` 负操作数编码，`expr.rs:317-322`）。
- **不存在字节级指令流**：`Instruction { op: OpCode(repr(u8)), operand: i32, loc: SourceLoc(3×i32) }`（`instruction.rs:5-10`）；产物以 JSON 承载，`op` 序列化为**变体名字符串**（`"PushConst"`）而非 u8 编号；跳转是**绝对槽下标**（`executor/control.rs:132-140`）。
- **跳转重定位**：发射期 `patch_jump` 直接改写 `code[ip].operand`（`lib.rs:594-598`）；跨模块拼接时由 `setup_vm` 对 `Jump/JumpIfZero/JumpIfNotZero` 统一加 `libc_code_len`（`native/src/engine/compile_pipeline.rs:254-263`）。
- **唯一指令构造点** `emit()`（`lib.rs:577-588`），同时维护 `source_map`（仅 `loc.line>0`）。

### 1.4 防线归属（本 crate 自测极薄）

- crate 内单测 **11 个**（`tests.rs`），只覆盖 `flatten_init_list` / `stmt_loc` / `compute_stride`，**字节码生成本体零单测**（`代码审阅与修复追踪20260906.md:93` 亦如此记载）。
- 真正防线在 crate 外：`native/tests/r1_memory_boundary_test.rs`（7 道）、`codegen_soundness_regression.c`（10 组断言）、`bytecode_gen_unit_test.rs`、shadow C 675（`shadow_c_cases`，as_of 2026-09-15）/ C++ 99（`shadow_cpp_cases`，as_of 2026-09-15）、E2E baseline 359 / K&R 81 / LeetCode 138 / gap 15（均 as_of 2026-09-18）、C++ E2E 83（`cpp_e2e_cases`，as_of 2026-09-18）。

---

## ② 可复用资产清单（标注移植成本）

| # | 资产 | 证据 | 成本 | 理由 |
|---|---|---|---|---|
| 1 | **132 条 opcode 语义表与编号** | `opcode.rs:23-157` | **低** | 纯数据表；编号手工显式赋值，可 1:1 落成 MoonBit `enum` 或"名字↔编号"表 + 穷尽 match |
| 2 | **Instruction 记录形态与 operand 编码约定** | `instruction.rs:5-10`；编码逐条：i32 直存 operand、f32 存位模式、f64/i64 存**池索引**、跳转存绝对 IP、局部/全局存偏移 | **低-中** | 形态是"3 字段定长记录"；成本在"约定"必须写成文档 + spike（§⑦ S1/S3） |
| 3 | **R1 内存布局规则（纯函数）** | `memory_state.rs:8-63`：`GLOBAL_START` / `HEAP_START` / `GLOBAL_REGION_LIMIT` / `compute_heap_base` / `argv_region_footprint` / `align4` | **低** | 已是纯函数 + 常量、零依赖；7 个 bump 站点（`lib.rs:335/443/467/485`、`expr/literal.rs:27`、`stmt/var_decl.rs:110/223`）语义可直接照搬为"布局计划" |
| 4 | **`bump_global_offset` 单一入口 + fail loud** | `lib.rs:604-626`（越界 `report_error` 并返 `None`；`generate` 末 `if !errors.is_empty() { Err }`，`lib.rs:555-557`） | **低** | 语义正确（编译期拒绝、不静默）；建议顺手结构化错误码 |
| 5 | **编译产物 schema（11 字段）** | `native/src/bin/vitro_cli.rs:303-316` | **低-中** | 字段清单可直接作为 MoonBit 侧契约；成本在"确定性序列化"（§⑧） |
| 6 | **Bytecode Libc 协作机制 + 预编译产物** | 固定索引段 base 1000 / 88 函数 / 用户从 1089 起（`bytecode_libc_index.rs:7-10`、`lib.rs:131-144`）；产物 3,485 条指令 / 88 函数（`bytecode_libc_data.json`）；`source_digest` 机制（`precompile_bytecode_libc/main.go:88-107,219-220,426-458`） | **中** | **产物本身语言无关，可直接复用**；机制（库模式、`with_mode` 预注册、`setup_vm` 拼接与 IP 偏移）需重写。**索引值不是稳定 ABI**——子勘察实证：更名前产物 `cide_*` 期索引相对当前整体 +1 漂移 |
| 7 | **调用约定与 ABI 设计** | 实参逆序压栈、`SplitD/SplitQ` 拆 64 位、隐藏返回指针 `__ret_ptr`（`func.rs:24-34`）、变参 `CallVar` + 真实 word 数（`call.rs:214-232`）、`PushArgv`+`PushArgc` 入口包装（`lib.rs:538-550`） | **中** | 设计只存在于代码里；迁移前必须补一份"调用约定规格"，否则无法做等价性验收 |
| 8 | **类型尺寸/布局口径** | `compute_type_size`、`elem_type_size`（`lib.rs:818-841`）、`ptr_step_size`（`lib.rs:804-816`）、packed struct 无填充（`lib.rs:851-858` 注释）、`type_align`（`lib.rs:855-899`） | **中** | 语义可复用，但散在 3 个函数 + 多处手写 match；应单源化为 `size_of/align_of/stride_of` |
| 9 | **栈缓冲区登记表（V-P1-6 教学安全检测输入）** | `stmt/var_decl.rs:51-61` → `LocalBuffer{offset,size,name}` → `func_meta.rs:5-13` → host 侧 E3070 检测 | **低** | 接口小、语义清晰，是教学价值的直接载体 |
| 10 | **布局回归测试 7 道** | `r1_memory_boundary_test.rs:78-282` | **低** | 断言文本与期望值可直接搬；仅需把驱动从 C ABI 换成新出口 |
| 11 | **Clang golden 语料** | `native/tests/cases/`（shadow C 675 as_of 2026-09-15 / C++ 99 as_of 2026-09-15） | **低** | 与实现语言无关（评估报告 §3.3 已判"最大可复用资产"） |
| 12 | **事故知识（§⑥ 的 39 条）** | 本报告 §⑥ | **低（但需人工搬运）** | 代码可重写，知识不会自动继承——即评估报告 §4 所指"真正的成本" |

---

## ③ 抛弃清单（Rust 特有机制 / 症状治疗代码 / 组织债）

| # | 抛弃对象 | 证据 | 抛弃理由 | 风险 |
|---|---|---|---|---|
| 1 | **`temp_slot0..3` 固定槽机制 + 3 个止血补丁**（`temp_slot_64` / `init_base_slot` / `assign_addr_slots`） | `lib.rs:63-81,686-735` | 8 条槽位事故（§⑥ #5/#7/#8/#9/#10/#11/#28 + ★B）证明在**占用过度 / 占用不足 / 物理相邻假设 / 跨函数生命周期**四个方向全出错；本质是"手工保证不冲突"的机制债 | **遗留面宽于修复面**：三个补丁只保护三类值，其余"跨 `gen_expr` 存活的中间值"仍在裸用共享槽（`代码审阅与修复追踪20260906.md:278` 原文） |
| 2 | **47 字段可变 `BytecodeGen` + 深度 `&mut self`** | `lib.rs:37-106` | Rust 借用检查逼出的"先 clone 再调用"（`lib.rs:248`、`init.rs:72`）在新语言无意义；47 字段互相隐式耦合是缺陷温床 | 拆成"只读上下文 + 可变发射器"会改变 API 形状，需重写全部调用点（≈6,200 行机械改动） |
| 3 | **`Vec<Instruction>` + `patch_jump` 事后回填** | `lib.rs:594-598`；`break/continue/goto` 三套补丁栈 | 绝对下标 + 事后回填，导致"跳转目标必须携带副作用语义"（RAII 析构范围，事故 #24）；回填越界**静默跳过**（`lib.rs:595` `if ip < len`）→ 零诊断 | 改成标签化两阶段 IR 会**改变产物字节形态**，必须与 §⑧ 锚点策略一起裁定 |
| 4 | **全量 `HashMap`/`HashSet` 驱动发射与序列化** | `lib.rs:40-105`；两次确定性事故（`CHANGELOG.md:1440-1443`；`main.go:399-403`） | 本项目**已栽过两次**：隐式移动构造生成顺序随进程随机种子变化；产物 JSON 键序随进程随机 | 换成有序 Map 会让"插入顺序=输出顺序"变成隐式契约；建议保留 Map 但**在序列化/发射边界统一排序** |
| 5 | **四张平行 `HashMap` 符号表** | `lib.rs:46-51`；`func.rs:11-17` 清理逻辑 | 同一实体分散四表，退出作用域需人工"回滚三元组"（`lib.rs:640-656`），漏一项即事故 #19/#33；`exit_function` 至今不清 `sym_index`（`func.rs:79-93`，评估 C7 未闭） | 改单一符号表需重写全部解析点 |
| 6 | **文本错误 `Vec<String>`** | `lib.rs:39,600-602`；`generate` 返 `Err(Vec<String>)` | 无错误码/无结构化字段，与项目其他层不一致；越界信息只能中文串匹配（`r1_memory_boundary_test.rs:222` 即 `msg.contains("全局数据区容量不足")`） | 结构化改造会改诊断文本 → 可能触碰现有断言与 facts 对账；建议新语言直接结构化 + 一次性映射兼容 |
| 7 | **字符串 mangling 即身份**：`"__ctor__{C}"` / `"__dtor__{C}"` / `"{C}__{m}"` / `"{}__size"` / `"{}__get"` | `lib.rs:474`、`struct_.rs:224`、`expr.rs:518`、`range_for.rs:61/131` | 靠字符串前缀判语义（配合 #8 启发式），漏改一处即事故 #29 同型；`CPP_FAILURES.md:69` 的"字节码等价测试永远 SKIP"正是 mangling 名字写错 | 需建立 Vitro 侧 mangling 函数 + 类型化符号；兼容期需译文层 |
| 8 | **字符串前缀启发式类型判定**：`is_lambda_closure_type`（`starts_with("__lambda_")`，`lib.rs:925-927`）、`is_builtin_container` + `starts_with("vitro_vec_")`（`range_for.rs:21-23`） | 同上 | 类型语义挂在"名字长什么样"上，typeck 侧改名即静默失效 | 影响 C++ 子集（评估报告 §3.2 未裁定），可与该裁定合并处理 |
| 9 | **死代码/占位/文档债**：`stmt/cpp.rs:19` `gen_try_stmt` 纯报错占位；`cpp/mod.rs` 仅 4 行 `mod`；`lib.rs:901-907` 重复两遍的空注释块；`opcode.rs:18` 注释"最大 opcode = PushArgv = 128"（实为 137）；`TrapBoundsVla` 无发射点 | 各处行号 | 零信息量或已失真 | 无（但 `TrapBoundsVla` 需先确认"有意保留兼容"还是"重构残留"——无注释可判，**待证**） |
| 10 | **`Option<i32>` + 调用方 `continue` 的错误传播** | `lib.rs:335-337/443-447/467-471/485-489` | 四处重复 `let Some(x) = … else { continue }`；错误已入 `errors` 但调用方继续发射（可能产半截产物），靠"最后统一失败"兜底 | 低（行为正确）；新语言应改 `Result`/错误累积器 |
| 11 | **单遍边生成边发射的组织形态** | `lib.rs:210-575` | 无 IR → 无法做"生成层栈平衡 / 槽位冲突"校验（评审建议 `代码审阅与修复追踪20260906.md:280`，**至今未落地**；`lib.rs:686-735` 无任何 `debug_assert`） | 引入 IR 会改变产物形态 → 与 §⑧ 锚点策略强耦合 |

---

## ④ 在途工作接纳方案

### 4.1 已登记的未完成项

| 项 | 内容与证据 | 新项目接纳方式 |
|---|---|---|
| **U3#1 作用域化槽位分配器** | `统一整备路线图.md:157`「固定 4 槽 → 作用域化分配器（bump + 释放 + debug 断言不重复占用）」；`:775-776` 2026-09-14 仍记"剩余登记"；代码现状 `lib.rs:686-699` 仍是固定 4 槽 | **直接按目标架构实现**。不迁移机制本身；把"槽位 = 显式 acquire/release 的 LIFO 池 + 不重复占用断言"列为新架构**必做项**（裁定判"不动"的依据是"3 起历史槽位 bug"，而实测清点到 **8 条**，样本量被低估） |
| **U3#3 `next_local_offset` 作用域回收** | `func.rs:21` 每函数重置、函数内只增不减；立项链 `:159`；定性 `:305`"容量放大而非正确性缺陷"（帧布局编译期固定、调用复用，无运行时泄漏） | **按目标架构实现**（帧大小受控是新语言免费收益）；但**会改 `FuncMeta.local_count` 与槽 offset → 破坏字节码逐位 diff**（§⑧ 风险 1） |
| **U3#7 变参实参区固定 64 字节** | `func.rs:54-57`「最多 16 个 int / 64 字节」；`:163` 原文"**>16 words 静默覆写被调函数局部变量（8 个 double 即到顶）**" —— **未修** | **直接按目标架构实现**：按实际字数动态预留 + 超限诊断。这是"固定常量给可变长数据划区"反模式，失败形态为静默覆写 |
| **U3#6 C++ 寄生收口 / U3#9 嵌套 struct mangling** | `:162`（§0 追踪表销项、mangled name 单源审计）；`:825-837`（嵌套名 mangled 化 `Outer__Inner` + 访问路径跟随"登记下批"） | **取决于 C++ 子集去留裁定**（评估报告 §3.2 未拍板）。若搬：**按目标架构实现**（新项目直接起 mangled 名，无历史名兼容负担）；若砍：**放弃并记录理由**（§③ #7/#8 随之消失） |
| **C7 `exit_function` 不清 `sym_index`** | `func.rs:79-93` 确证（只清 `local_indices`/`local_types`/`local_scope_stack`）；`重构评估报告20260912.md:77` 列为未闭 | **按目标架构实现**（单一符号表 + 作用域栈天然消除） |
| **D08 / D10 TODO 共 5 处** | `lib.rs:5`、`expr.rs:1`、`binary.rs:3`、`assign.rs:31`、`cpp/mod.rs:5` | **放弃并记录理由**：重写即消；拆分诉求来自 Rust 单文件认知负荷，与 MoonBit 包/文件组织无关 |
| **Layer A `Memcpy`/`Strlen` 未接 codegen** | `标准库架构与测试防线.md:294`、`:352`、`统一整备路线图.md:180` U5#5；实测 codegen 确不发射 | **待裁定**：先在新语言复现 profiling 再决定（`Memset` 已被 U1#5 接入，相关文档未同步） |
| **步数膨胀 P-3（未立项）** | `实测发现登记20260913_性能与头文件.md:119`（"codegen 逐 AST 节点直译是根因"）、`:355-357`（触"codegen 语义输出形态"保护区，"未立项前不动"） | **放弃（不继承）**：在新架构直接评估（IR 化 / 指令选择优化），不背旧形态做增量 |
| **`load_opcode/store_opcode` 收口、typeck↔codegen 单语义模型、codegen 语义回归层** | `代码审阅与修复追踪20260906.md:276-280`（§7.2 建议 6/8/9），**在 U0~U7 批次表中无对应行** | **直接按目标架构实现**（前两项是结构性收益；第三项的"槽位冲突 / 栈平衡 debug 校验器"应作为新项目**内建不变量检查**） |

### 4.2 本次新发现、**未登记**的活缺陷（须补登记）

| 缺陷 | 现象（实测） | 根因（file:line） | 接纳方式 |
|---|---|---|---|
| **★A 全局/静态 `char *p = "…"` 静默错值** | `char *p = "hi"; printf("[%s]\n", p);` → Vitro 输出 `[]`（`p = 0x6968`，即 'h''i' 字节被当作指针值）；**clang 输出 `[hi]`**。静态局部同形（`static char *p = "hi";` → 空） | `lib.rs:392-397` 的 `Expr::StringLiteral` 分支**不校验目标类型**，按 `sz` 逐字节写入（`char*` 的 `sz`=4）；typeck 侧全局标量初始化**不插隐式转换**（`vitro_typeck/src/lib.rs:325-352` 只 `check_assignable`，而局部路径 `decl.rs:288/321` 会 `insert_implicit_cast`），故字面量未被 `Cast` 包住而落进该分支 | **修复后搬 + 补防线**：① codegen 加目标类型判据（非 char 数组则报错或走 `gen_string_literal` 取址）；② 补 baseline `global_string_pointer.c` / `static_string_pointer.c`（**676 个现有用例零覆盖**：`^(static\|const)*char\s*\*+\w+\s*=\s*"` 命中 0）；③ 红→绿留痕 |
| **★B `gen_struct_copy` 目的地址槽被 `new[]` 覆盖** | `A b; b.v = 99; b = *new A[2]; printf("b.v=%d", b.v);` → Vitro `b.v=99`（赋值被**静默丢弃**，且改写了 `new[]` 块的 count 头）；**clang 输出 `b.v=1`** | `expr.rs:491` `dst_temp = get_temp_slot(1)` 先存目的地址；`expr.rs:475`（`gen_struct_copy_common`）随后 `gen_addr(right)`；右侧 `new A[2]` 走 `new_delete.rs:55` `ptr_temp = get_temp_slot(1)`——**同一槽**，在 `new_delete.rs:72` 覆盖目的地址。**指令级实证**（探针导出）：`StoreLocal 12`(dst) → `StoreLocal 12`(ptr_temp) → 拷贝循环写 `LoadLocal 12` | **修复后搬 or 由 U3#1 结构性消除**："槽位冲突家族"第 9 员，正落在裁定触发条件（`核心资产重构裁定.md:187`「≥2 起同类槽位冲突**且全防线放行**」）的描述面上——**五层防线全绿**。建议按 §⑦ S6 先立"槽位生命周期"规格，再决定"U3#1 落地后消解"或"新架构免检" |

### 4.3 时序风险（需重写方案正面回应）

`核心资产重构裁定.md:187` 的改判触发条件写作「**槽位分配器落地后** 6 个月内再现 ≥2 起同类槽位冲突且全防线放行」——**前置项（U3#1）未落地 ⟹ 计时起点未到达 ⟹ 现有判据在结构上无法对 codegen 槽位子域做出重写判定**。同时 U3 是 C# 引入的硬门禁（`统一整备路线图.md:249-254`）。对 MoonBit 重写的含义：**不要照抄机制，把"作用域化槽位分配器"列为必做项**。

---

## ⑤ 架构优化建议（MoonBit 形态）

**〔结构优化〕1. Layout Planner：把全局区从"7 个 bump 站点 + 隐式顺序"抽成纯函数**
现状是"谁先注册谁先占"的隐式顺序（`lib.rs:326-493`：全局变量 → extern 占位 → vtable → 回填字符串；`expr/literal.rs:27` 与 `stmt/var_decl.rs:110/223` 在生成期随时穿插 bump）。建议 `plan_layout(requests: Array[LayoutRequest]) -> LayoutPlan`，`LayoutRequest` 携带 kind/name/size/align/loc，`LayoutPlan` 给出 offset 表 + `global_data_end` + argv 区 + heap_base。收益：① 布局可单测（现靠 7 道集成测试间接覆盖）；② "字符串区与全局区重叠"（事故 #12）从"靠延迟分配约定"变成**类型上不可能**；③ `global_data_end` 与 `compute_heap_base` 的输入成为同一次规划的输出，消除双源。

**〔结构优化〕2. 作用域化槽位分配（LIFO 池），并把"持有"写进 API 形状**
可行性论证（基于本模块实测）：
- 这些槽**不是寄存器而是帧槽**——`get_temp_slot` 只做 `next_local_offset += 4`（`lib.rs:694-698`），访问方式是 `StoreLocal <立即数 offset>` / `GetFrameBase+PushConst+Add`；探针导出指令流实测 `StoreLocal 16`（两个 8 字节 struct 之后的第一个槽）。因此"分配"= 静态槽号指派，**不需要活跃性分析**：槽生命周期完全由发射嵌套决定（跨语句存活的只有 `switch` 的 cond，而 dispatch 段先于 case body 发射完毕，`switch.rs:41-66`）。
- 方向已被三次局部实践验证：`temp_slot_64`（独占 8 字节）、`init_base_slot`（路径专用）、`assign_addr_slots`（按嵌套深度分层）——三者都是"把隐式共享改成显式独占/分层"，各修掉一类真缺陷。**LIFO 池是这三者的统一形态**。
- **风险 1（最重要）：产物可 diff 性**。槽 offset 是指令立即数、帧大小进 `FuncMeta.local_count`，LIFO 复用会改变两者 → 与 Rust 版的字节码逐位 diff **必然全线飘红**。缓解：把"槽位分配策略"作为**显式版本化契约**（v1 = 逐位兼容现行策略，用于迁移期 diff；v2 = LIFO，迁移后切换），或把锚点降级为"槽位重编号后的语义等价"（§⑧）。
- **风险 2：释放点靠人肉维护即重演现状**。MoonBit 无 RAII，必须用回调式借用（`with_slot(fn(s) { … })`）或显式 acquire/release 对 + 断言（同层重复占用、未释放即越界）。只做复用不做显式化，冲突照旧。
- **风险 3：跨发射边界的持有必须显式化**（switch cond、struct copy 的 dst、`new[]`/`delete[]` 的 4 槽）。不显式化则 §⑥ 家族缺陷一件不少。
- **风险 4（待证）**：教学可视化/单步是否暴露槽 offset 语义（`source_map` 与符号表按 offset 定位）。若暴露，LIFO 复用会改变调试面板呈现。验证：查出口读取局部变量是否依赖符号 offset（`session_api.rs:527-539` 有 `GLOBAL_START + sym.addr` 用法，局部变量路径未逐点核实）。

**〔沿革保留〕3. opcode 表 + `Instruction` 记录 + 单 operand 槽**
132 条编号是产物/VM 的共同语言，也是 Clang golden 之外唯一"跨实现可比"的中间产物；**照搬编号而非重排**（重排会让历史产物与文档失效）。建议 `.mbti` 暴露 `OpCode` 与编号↔名字映射；VM dispatch 保持穷尽 match（Rust 侧即无 `_ =>` 兜底，`executor/mod.rs:82-244`）。

**〔新设计〕4. 两阶段发射：标签化 IR → 地址解析 → 产物**
现状 `patch_jump` 事后改写 + 三套补丁栈已在 RAII 析构范围上出过事故（#24），且回填越界静默跳过（`lib.rs:595`）。建议：基本块 + 符号标签 + **相对跳转**，收尾统一解析为绝对地址（或保持绝对但加"全部已解析"断言）。收益：跳转目标与副作用（析构范围）解耦；"未解析标签"成为可捕获错误而非静默 `Jump 0`。
**代价**：跳转编码形态变化 → 必须与 §⑧ 锚点策略同时裁定（建议迁移期先用"绝对 IP + 断言"保 diff，再优化）。

**〔结构优化〕5. 确定性发射器（Determinism by construction）**
本项目已两次栽在"Map 迭代顺序决定发射/序列化顺序"（`CHANGELOG.md:1440-1443`、`main.go:399-403`）。建议：① 一切"名字→实体"容器在**发射边界统一排序**（或使用有序容器）；② 序列化器显式定义 canonical form（键序、浮点位精确表示、行尾）；③ 把"同输入两次运行产物哈希相同"作为 **CI 断言**（现状只能人工连续跑 3 次观察，`CHANGELOG.md:1443`）。

**〔结构优化〕6. 单一符号表 + 类型化 mangling**
见 §③ #5/#7。`SymbolTable` 单源（名 → `{kind, ty, loc, addr, scope}`），mangling 为纯函数并单测，杜绝"字符串前缀当语义"。

**〔新设计〕7. 内建指令预算与不变量检查**
事故 #23 是 codegen 指令数 × JIT `MAX_TRACE_LEN=256` 的跨子系统隐式契约。建议把"单条语句/循环体指令数预算"作为生成器内建统计并 fail loud，同时内建"生成层栈平衡 + 槽位不重复占用"校验器（评审建议 6/8/9 至今未落地）。

**〔结构优化〕8. 诊断结构化**
`report_error` 产 `"第 N 行：文本"`（`lib.rs:600-602`），越界断言只能中文串匹配。MoonBit 侧用带错误码的 `Diagnostic`，迁移期用映射表兼容旧文本口径。

**〔新设计〕9. `.mbti` 契约化 `CompileOutput`**
把 13 字段的 `CompileOutput`（`lib.rs:929-947`）写成公共接口文件，作为"codegen ↔ VM/出口"的唯一契约；显式定义"哪些字段是产物、哪些是进程内契约"（现状有 5 个字段不进任何产物，§⑧ 8.2）。

---

## ⑥ 坑清单（现象 → 根因 → 修复 → 新语言是否复发）

**图例**：编号沿用本次勘察汇总序；★ = 本次新发现；"复发？"= 机制是否随语言更换消失。

| # | 现象 | 根因（file:line） | 修复 | 复发？ |
|---|---|---|---|---|
| 1 | T-P0-1：`double g=1` 打印 `0.000000` | 全局标量初始化按**字面量自身类型**编码位模式（`init.rs:33-34` 注释） | `literal_init_bits(expr,target_kind)` 单源（`init.rs:35-64`）+ `push_literal_init`（`:52-64`） | **是（高）**："覆盖哪条路径"没单源——全局（`lib.rs:340-424`）与静态局部（`var_decl.rs:131-265`）是**两套并行 match** |
| 2 | T-P0-2：`long long ga[2]={1,2}` 输出 `0 0`（局部正常） | `LongLiteral → (v as f64).to_bits()` | 同上 + 局部同构（`var_decl.rs:203-215`） | **是（高）**：同 #1；"全局修了局部没修"实际发生过 |
| 3 | T-P0-3 / T-P0-5：`double d++` 无效；`char cs[0]++` 进位污染邻元素 | `gen_mem_inc_dec` 全类型盲，固定 4 字节 `LoadMem/StoreMem+Add`（`unary.rs:11-13` 注释） | 按 `elem_kind` 分派 D/Q/Byte + `CastF2D/CastD2F`（`unary.rs:19-52`） | **是（中）**：清单型——每加一类型要同步分派 |
| 4 | T-P0-4：`gq.c='B'` 后相邻全局 `gnext` 变 0 | Member 赋值分支**缺 Char 臂**，写 4 字节越界（`assign.rs:45` 注释；5 组 match 全缺） | 补 `StoreMemByte/LoadMemByte`（`assign.rs:591-649`）+ `var_decl.rs:600` | **是（中）**：宽度决策散在 5~10 组手写 match；`load_opcode/store_opcode` 收口方案**未落地** |
| 5 | T-P0-6：`a[0] += (b[0]=5)` 得 `a0=1 b0=6` | 内外层赋值复用 `temp_slot0` | 嵌套深度计数 + 按深度分配槽（`assign.rs:33-37`、`lib.rs:723-735`） | **是（最高）**：深度计数**只覆盖 `gen_assign` 递归**，管不住 `gen_expr` 任意位置消费同一槽（★B 即其活体） |
| 6 | T-P0-7：两函数各 `static int x`，后者读到前者值 | `static_local_indices` 以**裸变量名**跨函数共享 | `enter_function` 清空（`func.rs:16-17`，注释 `:13-15`） | **是（中高）**：标识键无 mangling 化 |
| 7 | 8 字节位模式写踩踏邻槽，**9 个 baseline 链表/队列用例回归** | 4 字节槽承载 64 位值；且跨函数复用 offset 在小帧函数越界（`lib.rs:67-69`、`func.rs:68-69` 注释） | `temp_slot_64` 独占 8 字节 + 每函数重置（`lib.rs:703-710`） | **是（最高）**："槽位尺寸假设"+"生命周期假设"，与语言无关 |
| 8 | U1#3：`double a[3]={1.0, take(mk(4)), 3.0}` → trap「向 NULL 区写入(0x0010)」/ 数组越界假错 | 初始化基址存 `temp_slot0`，元素 `gen_expr` 内部覆盖基址（`lib.rs:70-76` 注释） | **仅止血**：`get_init_base_slot` 专用槽（`lib.rs:712-721`；落点 `var_decl.rs:374/393/544`）；根治在 U3 | **是（最高）**：止血面窄于残留面（任何"跨 `gen_expr` 存活的中间值"仍裸用共享槽） |
| 9 | lambda 实参位置 `StoreLocal` 冲出 1MB（`addr=1048576`） | 无捕获闭包 `size=0` → 帧内无槽位，而槽里存的是 4 字节地址（`var_decl.rs:36-46`、`lib.rs:918-927`） | `is_lambda_closure_type` 单一判定 + 保底 4 字节 | **是（高）**：根因方向与 #5~#8 **相反**（槽位**少算**）——两个方向都会错 |
| 10 | 变参 double/long long 跨槽写踩相邻局部变量 | `StoreLocalD slot0` 写 8 字节 + `LoadLocal slot0+4` 依赖**两槽物理相邻**，而惰性首次分配不保证相邻（`call.rs:178-181` 注释） | 迁 8 字节专用槽（`call.rs:182/194/366/378`） | **是（高）**："物理相邻"是隐式约定，任何新调用点都可能破坏 |
| 11 | `new[]`/`delete[]` 循环 `i_temp` 与 `user_ptr_temp` 冲突 | `get_temp_slot` 仅 3 槽（`CPP_FAILURES.md:80`） | 扩到 4 槽（`CHANGELOG.md:2278`） | **是（高）**："加槽"而非"作用域化"，冲突推给下一个组合 |
| 12 | kr_6_1：全局 `struct key keytab[]` 的 `char*` 成员被字符串内容覆盖 | 全局变量区与字符串区**共用同一起始地址**，Pass 1 中就埋入字面量（`KR_FAILURES.md:538`） | `pending_string_inits` 延迟到 Pass 1 后统一分配（`init.rs:119-122`、`lib.rs:481-493`） | **是（中高）**："多来源共用一个 bump 游标 + 无容量校验"未消除，只改了顺序约定 |
| 13 | `BYTECODE_LIBC_GLOBALS_RESERVED` 每次重生成膨胀 1KB → 用户全局区 <1KB → `lc_67` 溢出到堆 | library mode 下 libc 自身也从 reserved 起算，`globals_size` 回算 reserved = **自指闭环**（`lib.rs:176-188` 注释） | library mode 从 0 起算（15360 → 1024） | **是（中）**：生成物 + 派生常量的回算闭环，保留同型脚本即重建 |
| 14 | Bytecode Libc 产物被**文本替换**而非重生成 → CI 门禁必红；23 个函数固定索引重分配 | 目录更名改变源文件拼接顺序（`CHANGELOG.md:79-82`） | 以当前编译器重跑；归一化后 3,485 条指令多重集完全相同（`CHANGELOG.md:87-89`） | **是（中）**：产物即生成物，禁令需继承 |
| 15 | 带 argv 程序：argv 指针数组落在 `GLOBAL_START` 与全局数据重叠 | `setup_argv` 用 `global_count`（**恒 0**）编址（`CHANGELOG.md:1402-1403`） | 自 `GLOBAL_REGION_LIMIT` 向下分配 + 冲突 trap（`vitro_vm/core/state.rs:269-298`） | **是（中）**：线性内存模型前提不变 |
| 16 | 全局/字符串段（判据 `MEM_SIZE/16`）与堆（`HEAP_START`）重叠 → ">19KB 全局 + malloc"静默压坏堆 | 两套魔数、编译期不校验重叠（`CHANGELOG.md:1444-1449`） | **R1**：动态堆起点 `max(HEAP_START, align4(global_data_end))` + `GLOBAL_REGION_LIMIT` 单源 + fail loud | **低-中**：判据已单源；"三段共享 1MB"前提若保留则需重建同一约束 |
| 17 | `int a[2]={f(),3}` 编译成功但 `a[0]=0`（**静默错值**） | `flatten_init_list` 兜底 `_ => push(0)` + `unwrap_or(0)`（`重构评估报告20260912.md:74`） | 无条件 `gen_expr`（`var_decl.rs:501-528`） | **是（高）**："兜底值掩盖未处理形态"反模式，五层防线全绿 |
| 18 | `unsigned` 全线走 signed opcode → 「整数乘法溢出」trap | 未按 `is_unsigned` 分派 `UAdd/UMul/…`（`BYTECODE_LIBC_FAILURES.md:42`） | 新增 U 族 opcode + 三处同步（二元/复合赋值/JIT 模板） | **是（中）**：清单型，JIT 是第三个维护面 |
| 19 | 局部变量遮蔽全局数组 → **假边界陷阱** | `exit_scope` 只恢复 `local_indices`（`KR_FAILURES.md:347`） | 结构体三元组回滚（`lib.rs:15-21` 定义、`lib.rs:640-656` 回滚点） | **是（高）**：作用域回滚是系统性弱点；`exit_function` 不清 `sym_index`（C7）仍在 |
| 20 | `base_kind()` 递归解引用 → `char*[]`/`char**` 步长/宽度/初始化四点同时错 | 递归直到非指针（`KR_FAILURES.md:387-391`） | `immediate_base_kind()` 只解一层（`expr/unary.rs:177/293`） | **是（高）**：`base_kind` 语义自身不对称（对 Array 递归、对 Pointer 不递归） |
| 21 | `(*fp)(…)` 发射 `LoadMem` 读 NULL 区 | `Deref` 未区分 `TypeKind::Function` | Function 跳过 load（`unary.rs:181-183`） | **是（中）**："解引用即取内存"假设 |
| 22 | `return 2.5;` 在 double 返回函数输出 0 | 未插隐式转换 → `PushConstF` 而非 `PushConstD`（`LEETCODE_FAILURES.md:140`） | typeck 在 return 插 cast | **是（中高）**：与 #1 同族——"隐式转换插在哪"无单源（初始化在 codegen、return 在 typeck、赋值在 `gen_expr_with_cast`） |
| 23 | 循环体 `int a[12]` 指令爆炸 → JIT 注册半截 trace → **重放值栈溢出** | `emit_zero_init` 逐字节 `StoreMemByte`（5 指令/字节）× VM `MAX_TRACE_LEN=256`（`CHANGELOG.md:643-648`） | codegen 改 `Memset`（`var_decl.rs:624-634`）+ VM 录满改 Abort（`jit_trace.rs:90-95`，**代码已修**；`统一整备路线图.md:583` 仍写"仍未修"→ 文档滞后，见附录 A） | **是（中高）**：跨子系统隐式契约（指令数 × JIT 常量），任一侧改动都会破 |
| 24 | C++ RAII：break/continue 致 dtor 出现两次 → **假 E3061 Double-Free** | `emit_dtors_for_scope_exit` 的 `saturating_sub(1)` 长度/索引错位（`CHANGELOG.md:620-622`）；设计文档写 `start_frame_idx = target_depth`，实现却减一（**设计-实现漂移**） | 直接接收帧索引 + `loop_break_has_init_frame` 平行栈（`cpp/raii.rs:39-63`） | **是（中高）**：跳转目标需携带副作用语义；"先占位后回填"形态不消失就复发 |
| 25 | 多维全局数组嵌套初始化子元素大小算错（`int[2][3]` 子数组算成 4） | `elem_type_size` 递归取最内层类型 | 2026-06-06 修 | **是（中高）**：与 #20 同族（按类型求宽度的递归语义） |
| 26 | 模板 `infixEvaluation_default` 报 `valStack[-1]` 越界，**长期被误记为"模板自身栈下溢"** | 真因是自增/自减作数组索引的 codegen 缺陷（`E2E_FAILURES.md:38`、`代码审阅与修复追踪20260906.md:48`） | 同 #3/#5 | **是（语言无关）**："把编译器缺陷归因到用户/模板代码"的诱惑 |
| 27 | 字节码等价比较测试**永远 SKIP**，从未执行 | mangling 键写错（`"get__vector__int"` vs `"vector__int__get"`，`CPP_FAILURES.md:69`） | 修正键名 + 严格断言 | **是（中）**：依赖外部命名字符串的断言会静默失效（J9 适用面） |
| 28 | 嵌套 `new` 覆盖 `temp0` | `new_delete.rs:149` 单对象 `new` 取 `get_temp_slot(0)`；`代码审阅与修复追踪20260906.md:177` 登记 | **未见修复记录**（子勘察通读未见专用槽注释）→ **待证** | **是（最高）**：同 #5 家族；台账原话已把它算作"3 起同类 bug"之一 |
| 29 | `gen_addr` 不支持 static 局部：`static struct S s1; struct S s2 = s1;` 报"未声明的变量" | `expr.rs:383-408` 的 Identifier 分支只查 `local_indices`/`global_indices`，**无 `static_local_indices` 分支**；而 `unary.rs:116-119` 的 `&x` 路径**有** | **确认未修**（`代码审阅与修复追踪20260906.md:176` 登记） | **是（中高）**："同一语义存在多条并行实现，修一条漏一条"（与 #4 同型） |
| 30 | `struct S{int a,b,c}; struct S s={1};` 尾字段是否清零（C 要求零初始化） | designator 路径**有** `Memset` 预清零（`var_decl.rs:459-464`/`:551-556`），非 designator 路径**无**且 struct 路径 `break`（`:575-583`）——**自身不对称** | **登记未修**（`代码审阅与修复追踪20260906.md:172`）；是否被 `emit_zero_init`（`:612-636`）兜住 → **待证** | **是（中高）**：把"补齐/清零"挂在某条初始化分支而非统一入口 |
| 31 | 变参实参区固定 64 字节 → **>16 words 静默覆写被调函数局部变量（8 个 double 即到顶）** | `func.rs:54-57` 固定预留（`统一整备路线图.md:163`，来源 vm/runtime 审查 P0-3） | **未修**（挂 U3#7） | **是（高）**："固定常量给可变长数据划区"+ 静默覆写（最坏失败模式） |
| 32 | 全局/静态 designated initializer **实际不支持**，而文档曾称 Phase 30 已支持 | `lib.rs:343-347`、`var_decl.rs:134-138` 明确 `report_error`（**行为诚实，文档口径不诚实**） | — | **元教训**：文档声称的能力 ≠ 实现的能力 |
| 33 | `&&`/`\|\|` 结果不规范化（`5&&3` 得 3）；struct 按值拷贝 `size/4` 截断（`{char a;char b}` 拷 0 字节） | `binary.rs:50-76`；`size/4` vs `(sz+3)/4` **三处不一致**（`expr.rs:471`、`control.rs:140`、`call.rs:53`） | 短路末补 `PushConst 0 / Ne`（`binary.rs:76` 注释）+ 统一取整 | **是（中高）**：尺寸计算散写 = 单源缺失 |
| **★A** | **全局/静态 `char *p="hi"` 静默错值（`p=0x6968`）** | `lib.rs:392-397` 不比目标类型按 `sz` 写字节 + typeck 全局标量初始化不插 cast（`vitro_typeck/lib.rs:325-352` vs `decl.rs:288/321`） | **未修（本次新发现；676 用例零覆盖）** | **是（高）**：类型判据缺失 + "初始化路径两侧不对称" |
| **★B** | **`b = *new A[2]` 赋值被静默丢弃 + 改写 `new[]` 的 count 头** | `expr.rs:491`（dst = slot1）与 `new_delete.rs:55`（ptr = slot1）争用；指令级实证 `StoreLocal 12` 两次 | **未修（本次新发现；五层防线全绿）** | **是（最高）**：槽位家族第 9 员，正落在裁定的重写触发描述面 |

**四条结构性判断（给新语言的不变量）**：

1. **"手工固定槽 + 人工保证不冲突"两个方向都错过**：占用过度（#5 / #8 / #11 / #28 / ★B）、占用不足（#9）、物理相邻假设（#7 / #10）、跨函数生命周期假设（#7）。→ 必须显式化。
2. **"类型 → 宽度 / opcode / 步长 / 位模式"的决策没有单源**（#1 / #2 / #3 / #4 / #18 / #20 / #22 / #25 / #33）。→ 收敛为 `size_of / align_of / stride_of / load_op / store_op` 等少数纯函数。
3. **codegen ↔ VM/JIT 的隐式契约是第二复发源**：指令数 × JIT 上限（#23）、槽 offset × 帧布局、全局偏移 × `global_data_end` × heap_base（#16）、符号索引 × `TrapBounds`（#19）。
4. **"静默"是本模块最贵的失败模式**：初始化兜底 `unwrap_or(0)`（#17）、无匹配臂 `_ => {}`（`init.rs:123-124`）、`patch_jump` 越界静默跳过（`lib.rs:595`）、`get_temp_slot` 越界回退 slot0（`lib.rs:692`）、★A / ★B。**五层防线对其中多数未作拦截。**

---

## ⑦ MoonBit spike 清单

> MoonBit 事实分三级：**已验证**（本机 `moon 0.1.20260915` + `moon ide doc`，命令随附）、**待核**（给出核实命令）、**未知**（给出获取路径）。不把未核实项写成事实。

**S1 · opcode 表示与编号映射**
- 依赖特性：`Byte`/`UInt` 表示、fieldless enum、穷尽 `match`。
- 最小程序：定义 132 条 opcode 表（编号 0..137 含 6 个空号），实现 `op_from_u8(UInt) -> OpCode?` 与 `op_name(OpCode) -> String`；跑 0..255 全空间断言"空号与越界返回 None"。
- 判定标准：① 全空间无 panic；② 名字↔编号双向一致（132 条全覆盖）；③ 新增 opcode 时编译器强制暴露未覆盖 `match` 分支。
- 已核实：`Byte` / `FixedArray[T]` / `Int` 类型存在（`moon ide doc 'Bytes'` / `'FixedArray'` / `'Int'`）。

**S2 · C 字节流 vs MoonBit String（最易踩的语义陷阱）**
- 依赖特性：`String` 内部是 UTF-16；`Bytes` 不可变。
- 最小程序：把 C 源码字面量当**字节流**处理——`"héllo中"` 的字节长度、按字节写入 char 数组、`strlen` 语义。
- 判定标准：`char s[] = "héllo中"` 的数组内容与字节数**逐字节等于 Clang**；**禁止** `String::length` 参与任何"字节数"计算。
- 已核实（**直接决定迁移正确性**）：`moon ide doc 'String::length'` → "Returns the number of **UTF-16 code units** in the string"；`String::to_bytes` 文档写明 "String holds a sequence of **UTF-16 code units**" 且该方法**已 deprecated**。对照 Rust 侧现状：`lib.rs:394` 用 `value.as_bytes()[i]`、`expr/literal.rs:23` 用 `value.len()`（**UTF-8 字节**）——直接照抄 `String` 会在非 ASCII 上静默错值。

**S3 · i32/i64/f64 常量池的位精确编码与 JSON 往返**
- 依赖特性：浮点位模式转换、`Double`/`Int64` 表示、JSON 库。
- 最小程序：构造含 `3.141592653589793` / `1e-308` / `f64::MAX` / `-0.0` / `NaN` 的 double 程序与 `i64::MIN/MAX`，走一遍"池化 → 序列化 → 反序列化 → 比位模式"。
- 判定标准：往返后 `to_bits()` 全等；`-0.0` 与 `NaN` 不被规范化；池去重语义与 Rust 一致（**注意口径差异**：f64 用 `to_bits()` 去重 `lib.rs:787`，i64 用值比较 `lib.rs:796`）。
- 待核：JSON 库与浮点往返精度（`moon ide doc '*json*'`；`moon add` 需联网，离线请以本地 mooncakes 缓存为准）。

**S4 · Map 迭代顺序与"确定性发射"**
- 依赖特性：`Map`/`HashMap` 迭代顺序是否稳定（同进程内、跨进程）。
- 最小程序：建 100 条 `String→Int`，连续两次独立进程打印迭代序列哈希。
- 判定标准：**跨进程哈希相同**；若不同，则禁止用迭代顺序决定发射/序列化顺序（本项目已有两次真实事故：`CHANGELOG.md:1440-1443`、`main.go:399-403`）。
- 待核：`moon ide doc 'Map'`（迭代顺序未验证，不做断言）。

**S5 · 全局区布局纯函数（Layout Planner 原型）**
- 依赖特性：整数宽度与溢出语义、`Int`/`UInt`/`Int64` 转换。
- 最小程序：实现 `plan_layout(requests) -> {offsets, global_data_end, argv_base, heap_base}`，覆盖 7 个 bump 类型 + argv 向下分配 + 越界 fail loud；把 `r1_memory_boundary_test.rs:257-282` 的 7 条断言逐条搬来。
- 判定标准：与 Rust 版 `compute_heap_base`/`GLOBAL_REGION_LIMIT` 在同输入下**数值全等**；越界必报错（不得静默）。
- 已核实事实基线：`GLOBAL_START=0x1000` / `HEAP_START=0x5000` / `GLOBAL_REGION_LIMIT=MEM_SIZE/16=0x10000` / `MEM_SIZE=0x100000`（`memory_state.rs:4-18`）。

**S6 · 帧槽 LIFO 分配器 + 生命周期不变量**
- 依赖特性：可变数组、显式资源释放（无 RAII）、断言。
- 最小程序：实现 `with_slot(fn(slot) { … })` 形式的作用域化槽池（bump + release + debug 断言不重复占用），把 §⑥ 的 8 条槽位事故各写一个回归（尤其 ★B）。
- 判定标准：① 8 条回归全部通过；② 深嵌套表达式压力用例（`f(&t)` 1024 层）确定性报错而非崩；③ 断言可被"人为注入冲突"触发（J9 埋雷义务）。
- **必须同时裁定**：槽 offset 是否进入"逐位 diff"契约（§⑧ 风险 1）。

**S7 · 132 路穷尽 match 的工程可维护性**
- 依赖特性：`match` 穷尽检查、`enum` 携带数据。
- 最小程序：把 `Instruction` 的 operand 语义（i32 立即数 / f32 位模式 / f64 池索引 / i64 池索引 / 全局偏移 / 帧偏移 / 函数索引 / host id / 绝对 IP / 行号 / 符号索引）编码成带数据的 enum，写全分支 `decode`。
- 判定标准：新增 operand 变体时编译期暴露全部遗漏点；`decode` 对 132 条 opcode 全覆盖且无 `_` 兜底。

**S8 · 产物序列化与 canonical form**
- 依赖特性：JSON 库、数字保真、字符串编码（**切忌 UTF-16 泄漏到字节流**）。
- 最小程序：把 §⑧ 的 11 字段 schema 序列化两次（同输入、独立进程），比对字节。
- 判定标准：**字节级一致**；键序显式定义（不依赖 Map 顺序）；`op` 以**变体名字符串**输出（与现存产物兼容，`bytecode_libc_data.json:9`）。
- 待核：JSON 库 API 名（`moon ide doc '*json*'`）。

**S9 · 整数溢出与除法语义**
- 依赖特性：`Int` 溢出行为（wrap / trap）、`/` 与 `%` 对负数、`checked_mul`。
- 最小程序：`compute_stride`（`init.rs:200-237`，Rust 用 `checked_mul` 返 0 哨兵）的 MoonBit 版 + 有符号右移/取模对照表。
- 判定标准：与 Rust 版逐值一致；溢出路径显式（不得依赖语言默认 wrap 静默通过）。

---

## ⑧ 等价性验收锚点

### 8.1 最强锚点：产物序列化逐字节 diff（可行，但**必须先固定确定性**）

**已验证的确定性现状（本次探针实测，同一份源码连续两次 `vitro_cli export`）**：
- `code` 段（指令序列）**逐字节相同**（3,112 字节，SHA256 相同）；
- 整体文件 SHA256 **不同**，文件长度相同（6,526）；
- 排序后行集合**仅差 4 行**，差异全部是"**最后一个键少一个尾逗号**"（`"__ctor__vitro_list_node__int__move": 1080,` vs `…: 1080`）——即**唯一的不确定性是 HashMap 键序**。

**代码层根因（非推测）**：`native/src/bin/vitro_cli.rs:308-309` 把 `func_table: HashMap<String, FuncMeta>`、`func_index: HashMap<String, i32>` 直接交给 serde_json；`scripts/precompile_bytecode_libc/main.go:399-403` 注释原文：「产物由 Rust 侧 HashMap 派生，其序列化键序**随进程随机种子变化**……」。历史上已用 `sort_keys`（Go/Python）掩盖，并在 `CHANGELOG.md:1440-1443` 修过同族问题（`generate_implicit_move_ctors` 遍历 `HashSet`）。

**"逐字节 diff"的成立条件（建议写成迁移期硬约束）**：
1. 比对前**双方都 canonical 化**：键序（字典序）、行尾（LF）、浮点（最短往返表示或直接比位模式）；
2. Rust 侧**保留现行槽位分配策略**（否则 `StoreLocal <offset>` 立即数与 `FuncMeta.local_count` 全线变化——§⑤ 风险 1）；
3. 库模式（`--builtin-libc`）与用户模式分开比对；
4. `source_digest` 字段**排除**（构建期元数据；`main.go:219-220` 写入，Rust 结构体无此字段，`bytecode_libc_loader.rs:12-25`）。

**工具**：`scripts/precompile_bytecode_libc`（Go）已有 `sort_keys` 语义，可抽成独立 canonicalizer；或 `jq -S` / 自写 Go 比对器（项目脚本默认语言为 Go）。

### 8.2 分层对照表

| 层 | 对照物 | 格式 | 工具 |
|---|---|---|---|
| L1 指令流 | `code` 数组（`{op 名, operand, loc{line,column,file_id}}`） | JSON | canonical 后逐字节 diff |
| L2 函数元数据 | `func_table`（**注意 export 会移除 `main`**，`vitro_cli.rs:322`）+ `func_index` + `globals_size` | JSON | 按 `func_index` 值排序后 diff |
| L3 数据段 | `globals_init_32` / `globals_init_64` / `string_data`（**绝对地址**）/ `f64_constants` / `i64_constants` | JSON | 逐元素 diff；f64 比位模式 |
| L4 **未进产物但必须验的 5 字段** | `source_map` / `symbols` / `struct_defs` / `union_defs` / `global_data_end`（`CompileOutput` 13 字段中的未导出者） | **需新增调试导出**（建议 `--dump-compile-output`） | 逐元素 diff。**当前无法 diff 这 5 项 → 建议列为迁移期工具建设项** |
| L5 端到端 | Clang golden stdout（shadow C 675 as_of 2026-09-15 / C++ 99 as_of 2026-09-15；E2E baseline 359 / K&R 81 / LeetCode 138 / gap 15 as_of 2026-09-18） | 文本 | `go run ./scripts/shadow_verify`（驱动需重建，评估报告 §3.3 已列） |
| L6 布局规则 | `r1_memory_boundary_test.rs` 7 道断言 | Rust 断言 → 新语言测试 | 断言文本直接搬 |
| L7 库自举 | `bytecode_libc_consistency`（12 用例，`bytecode_libc_consistency.rs:142-203`） | 断言 | 产物**可原样复用**（3,485 条指令 / 88 函数），只需新 VM 能加载 |

**三条"必须冻结"（决定 diff 能否成立）**：
1. **槽位分配策略**（temp slot offset 进指令立即数；探针实证 `StoreLocal 16`）；
2. **跳转编码**（绝对 IP；跨模块由 `setup_vm` 加 libc 长度偏移，`compile_pipeline.rs:254-263`）；
3. **Bytecode Libc 固定索引**（1000 + 按原始索引排序的位次；**历史已漂移过一次**）→ 索引值**不是稳定 ABI**，diff 时建议按"名字→索引"映射比对而非按数值。

### 8.3 执行建议

- **每搬一个 crate 做差分扫描**（评估报告 §7 阶段 1 已定）；codegen 的差分输入应是**全量 Clang golden 源码**（1,888 份用例文件量级），而非抽样；
- 差分产物应**同时包含 L1~L4**（L4 需先补导出能力）；
- 把"两次运行产物哈希相同"纳入 CI（现状仅人工连续跑 3 次验证，`CHANGELOG.md:1443`）。

---

## ⑨ mooncakes 包切分草案

**切分原则**：按"变更频率 × 依赖方向"切，不按当前文件树切（当前 `cpp/` 与 `stmt/` 的划分是 Rust 模块债，不是领域边界）。

```
vitro/codegen/ir           # 纯数据层：OpCode(132) + Instruction + operand 语义校验
                           #   + SourceLoc/FuncMeta/Symbol/LocalBuffer + CompileOutput 契约（.mbti）
                           #   依赖：无（仅 core）
vitro/codegen/format       # canonical 序列化/反序列化（11 字段 schema）+ schema 版本
                           #   依赖：ir
vitro/codegen/layout       # Layout Planner：全局区/字符串区/vtable/extern 占位/argv/global_data_end/heap_base
                           #   依赖：ir
vitro/codegen/frame        # 作用域化槽位分配器 + 指令预算 + 生成层不变量检查器（栈平衡/槽冲突）
                           #   依赖：ir
vitro/codegen/c            # C 子集语义 → 发射（宽度单源 load_op/store_op、初始化编码单源 literal_bits）
                           #   依赖：ir, layout, frame
vitro/codegen/cpp          # C++ 扩展（类/RAII/虚表/模板单态化/new-delete/lambda）—— **待裁定搬或砍**
                           #   依赖：c（复用发射原语）
vitro/codegen/libc         # Bytecode Libc 固定索引段 + 预编译产物加载 + source_digest 校验
                           #   依赖：ir, format
vitro/codegen/conformance  # 差分驱动器：Rust 产物读取 + canonical diff + Clang golden 跑批（迁移期专用，可不发布）
                           #   依赖：format, c
```

**依赖方向**：`ir ← {format, layout, frame, libc} ← c ← cpp`；`conformance` 只读全部，**无环**（现状 `vitro_codegen → vitro_runtime / vitro_cpp_frontend` 的单向依赖已在 `Cargo.toml:7-10` 成立，可平移）。

**对上发布形态**：
- 公共接口用 **`.mbti`** 显式声明：`ir`（`OpCode`/`Instruction`/`CompileOutput`）、`format`（`encode`/`decode`/schema 版本）、`c`/`cpp`（`compile(…) -> CompileOutput` + `Diagnostic`）、`layout`（`plan_layout`）。
- **不发布**：`frame` 的内部槽位策略细节（仅暴露"策略版本"常量以便 diff 对齐）、`conformance`、`layout` 内部探针。
- 版本化：产物 schema 版本（当前硬编码 `version: 1`，`vitro_cli.rs:369`）与 libc 索引段（`BYTECODE_LIBC_*`）应作为**独立版本号**随包发布，避免"索引漂移"再次以隐式方式发生。
- 测试形态：`_test.mbt`（黑盒：ir/format/layout 的纯函数性质）+ `_wbtest.mbt`（白盒：frame 的槽位不变量、c 的发射序列）。**codegen 生成本体必须自建单测层**——现状 crate 内仅 11 个工具函数测试，全部缺陷靠外部防线间接拦截。

---

## 附录 A · 增补与勘误（子勘察全部回收后的精化）

### A.1 已独立复核的文档-代码口径漂移（对 §④ 的影响）

- **代码**：`native/crates/vitro_vm/src/jit_trace.rs:90-95` —— `if self.instructions.len() >= MAX_TRACE_LEN { … return RecordResult::Abort; }`（仅 `:107` 正常路径返回 `Finish`）→ **已修**；红锚 `native/tests/cases/baseline/jit_trace_overflow_abort.c:1-4`（golden 来自 clang，`total=107400`）在位。
- **文档**：`docs/current/07-质量与裁定/统一整备路线图.md:583` 原文「（`jit_trace.rs:90-93`，**仍未修**，半截 trace 栈不平衡问题照旧在册）」→ **滞后**。
- 另一处同类漂移（子勘察报告，**本报告未逐字复核 → 待证**）：`docs/current/07-质量与裁定/INCIDENTS/事故202609_Seek重放泄漏.md:8` 仍写"复发洞未修复"，而路线图已记 U2#1 收口。

**对重写方案的含义（比漂移本身更重要）**：新项目继承的是**文档化知识**，而文档已出现"已修记成未修"的漂移。因此 §⑧ 的锚点不能建立在"文档声称的修复状态"上，只能建立在**当轮跑出的产物 diff + golden** 上；§④ 的在途清单也应以**代码现状**为准（本报告 §④ 已按此执行：#31 / #28 / #29 / #30 四条均为代码亲读确认未修，而非引文档）。

### A.2 §⑥ 三处状态精化

| 原表项 | 精化 |
|---|---|
| #23（指令爆炸 × JIT 半截 trace） | 状态精化为"**代码已修 + 路线图文档滞后**"（证据见 A.1） |
| #31（变参实参区固定 64 字节） | 补登记编号 **U3#7**（`统一整备路线图.md:163`，来源"vm/runtime 审查 P0-3"）；**未修** |
| #30（局部初始化尾字段不清零） | 补不对称证据：designator 路径 `var_decl.rs:459-464`（数组）与 `:551-556`（struct）**有** `Memset` 预清零；非 designator 路径 `:501-528`（数组）无、`:575-583`（struct）**`break` 后完全不动尾字段**；是否被 `emit_zero_init`（`:612-636`）兜住 → **待证** |

---

## 附录 B · 待证清单

| # | 待证事项 | 验证方法 |
|---|---|---|
| 1 | ★A / ★B 在 HEAD 源码是否仍存在 | `cargo build --release --bin vitro_cli` 后重跑两条探针（源码见下） |
| 2 | 嵌套 `new` 覆盖 `temp0`（#28）是否已修 | 探针 `A* p = new A(new B());` + grep `new_delete.rs` 专用槽注释 |
| 3 | `gen_addr` 缺 static 分支（#29）的**可达性** | 探针 `static struct S s1; struct S s2 = s1;`（台账原形状）/ `static int x; int *p=&x;` |
| 4 | 局部 struct/数组初始化尾字段是否清零（#30） | 探针 `struct S{int a,b,c}; struct S s={1}; printf("%d %d %d")` + `int m[2][2]={{1}}`，与 clang 对拍；并读 `var_decl.rs:71-95` 确认 `emit_zero_init` 与带 init 路径的分支顺序 |
| 5 | `KR_FAILURES.md:368` 的 `char*[]` 残留（`parr[0/1]` 读值错） | 探针 `char *parr[]={"a","b","c"}; printf("%s\|%s\|%s")` 对拍 clang（`kr_5_11`） |
| 6 | `size_of::<Instruction>()` 是否为 20 | 由 Rust 布局规则推导（1B op + 3B padding + 4B operand + 12B loc），未编译验证 |
| 7 | `f64_constants` JSON 往返位精度是否无损 | 含 `3.141592653589793` / `1e-308` / `f64::MAX` 的程序 export → `--check` 重生成比对字节 |
| 8 | `runtime_libc/include/*.h` 不在 `source_digest` 覆盖面内（`main.go:50,64-86` 只扫 `src`/`vitro`） | 改一个 `include/*.h` 声明 → 重建 CLI → 重跑 export → 看 `--check` |
| 9 | `BYTECODE_LIBC_CODE_LEN`(3485) 与产物 `code.len()` 的双源风险（无校验） | 把常量改成 3484 跑 `vitro_cli step`，观察是否无断言拦截 |
| 10 | U3#1 / #3 / #6 / #7 / #9 是否在 2026-09-15 后动工 | 读 2026-09-18 之后的文档/代码快照（本次证据截止 09-15/09-18） |
| 11 | `native/tests/*FAILURES.md` 两处状态矛盾（`CORE_ASSET_VERDICT_FAILURES.md:152` OPEN vs `CHANGELOG.md:843-858` 已修；`KR_FAILURES.md:368` vs `:474-486`） | 跑 `jit_path_parity` 与全量防线后回填台账 |
| 12 | `TrapBoundsVla`(129) 是"有意保留"还是"重构残留" | 无注释可判，需作者确认或查设计文档（非 archive） |
| 13 | LIFO 槽位复用是否影响教学可视化 | 查出口读取局部变量是否依赖符号 offset（`session_api.rs:527-539` 为全局路径，局部路径未逐点核实） |
| 14 | MoonBit JSON 库 API、`Map` 迭代顺序、整数溢出语义 | `moon ide doc '*json*'` / `'Map'` / `'Int'`（本机 `moon 0.1.20260915` 可跑） |

**★A / ★B 复现源码**（`vitro_cli run -` 或临时 `.cpp` + `vitro_cli run <file>`；clang++ 对照需自行加 `#include <cstdio>`）：

```c
/* ★A */
char *p = "hi";
int main(){ printf("[%s]\n", p); return 0; }      /* Vitro: []      clang: [hi] */

/* ★B（需 .cpp 扩展名以启用 C++ 前端） */
class A { public: int v; A(){ this->v = 1; } };
int main(){ A a; A b; b.v = 99; b = *new A[2];
            printf("b.v=%d\n", b.v); return 0; }  /* Vitro: b.v=99  clang: b.v=1 */
```

---

## 附录 C · 勘察方法与诚实性声明

1. **只读边界**：仓库内**零文件改动、零 git 命令**（含 `git status`）。所有探针产物写在系统 `%TEMP%`：`vitro_probe_det/`（2 个 export JSON、3 个 `.cpp`、1 个 clang 产物）、`vitro_moon_probe/scratch/`（MoonBit 能力探针工程）。
2. **探针所用二进制**：`native/target/release/vitro_cli.exe`（构建时间 2026-09-15 00:05）与 `native/target/debug/vitro_cli.exe`（2026-09-18 13:01）。★A / ★B 在两个二进制上均复现；**未重新构建**，故"HEAD 源码现状"标待证（附录 B #1）。
3. **clang 对照**：本机 `clang++ 22.1.4`（`C:\clang+llvm-22.1.4-x86_64-pc-windows-msvc`）。★B 的 golden `b.v=1` 由 clang++ 实跑取得；★A 的 golden `[hi]` 由 C 语义直判（clang 实跑未做，**待证**）。
4. **证据优先级**：代码行 > CHANGELOG > 事故台账 > 评估/裁定文档。凡文档与代码冲突，以代码为准并标出冲突文档行（附录 A.1）。
5. **全局数字口径**：一律读 `reports/facts.json`（`generated_at` 2026-09-18T01:36:48+08:00，`git.rev` `4b57191`）并标 `as_of`；本报告内的规模/计数为本次实测快照，冻结不回填。
6. **未引用**：`docs/archive/` 下任何文档全程未读、未引用。
7. **子勘察**：4 路并行只读子勘察（指令发射与产物格式 / R1 内存布局 / 在途工作与债务 / 事故史 / C++ 扩展面），其结论已尽可能回到源文件复核；本文中标注"待证"者即未完成独立复核的项。
