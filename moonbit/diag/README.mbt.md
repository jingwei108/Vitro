# vitro/engine/diag — Vitro 诊断契约

C 教学引擎的诊断错误码契约：**137 个错误码**（1xxx 词法 / 2xxx 语法 / 3xxx 语义 / 4xxx C++ 专属）+ 三档严重级 + 77 条教学卡片。码表由 Rust oracle 源生成（禁手抄），与 Vitro 引擎的诊断输出逐字节对齐。

## 安装

```bash
moon add vitro/engine/diag
```

## 三个上下文

### 1. 教学反馈：把错误码变成讲解

```mbt check
///|
test {
  let code = @diag.ErrorCode::E3007_StringInitNonCharArray
  inspect(code.display_code(), content="E3007")
  inspect(code.severity().to_str(), content="error")
  match code.catalog() {
    Some(card) =>
      inspect(card.title, content="字符串初始化非字符数组")
    None => fail("E3007 应有教学卡片")
  }
}
```

### 2. 前端着色：按严重级分流显示

```mbt check
///|
test {
  // W3053（隐式标量转换）是警告——提示而非报错的前端语义
  let w = @diag.ErrorCode::W3053_ImplicitScalarConversion
  inspect(w.severity().to_str(), content="warning")
  inspect(w.display_code(), content="W3053")
  // H3057（隐式转换提示）是 hint——信息级
  let h = @diag.ErrorCode::H3057_ImplicitConversionHint
  inspect(h.severity().prefix(), content="H")
}
```

### 3. 工具链：整目录导出（与 Rust oracle 逐字节对拍）

```mbt check
///|
test {
  let json = @diag.export_catalog_json()
  // {"catalog":[{...},…]} 码升序 77 条——与 Vitro Rust 版 error_catalog
  // 出口经 canonicalize 归一后逐字节一致（E4 锚）
  assert_true(json.has_prefix("{\"catalog\":["))
  assert_true(@diag.catalog_codes().length() == 77)
}
```

## 契约要点

- **码位只增不改**（versioned 常量语义）——`code` 输出是稳定 ABI；
- 穷尽 match 无兜底臂：新增码在依赖方重新编译时立即暴露；
- `catalog()` 对无卡片码返回 `None`，无卡片清单可用 `codes_without_catalog()` 断言；
- `lang()`：4xxx → `Cpp`，其余 → `C`（`CSharp` 变体为 5xxx 预留段先行落位）。

## 生成物说明

`error_code_gen.mbt` / `catalog_gen.mbt` 由 `moonbit/scripts/gen_diag` 生成（**禁手改**，文件头有源 sha256 落款）。
