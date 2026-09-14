//! Vitro 类型检查器。
//!
//! 从 `vitro_native::compiler::typeck` 拆分而来，负责 C/C++ 教学子集的语义检查与类型推断。

// TypeChecker 入口模块。核心检查流程保留在此，作用域/转换/初始化等逻辑已下沉到子模块。
use vitro_ast::*;
use vitro_shared::ErrorCode;
use std::collections::{HashMap, HashSet};

mod context;
mod convert;
mod init;
pub(crate) mod symbols;

pub(crate) use convert::{apply_default_argument_promotions, insert_implicit_cast};
pub(crate) use symbols::*;

#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub line: i32,
    pub column: i32,
    pub code: i32,
}

pub struct TypeChecker {
    /// 库模式：当前正在预编译 Bytecode Libc 本身，不应再混入 builtin_layout 预注册类。
    pub is_library_mode: bool,
    pub(crate) errors: Vec<TypeError>,
    pub(crate) warnings: Vec<TypeError>,
    pub(crate) hints: Vec<TypeError>,
    pub(crate) funcs: HashMap<String, FuncSymbol>,
    pub(crate) static_func_sigs: HashMap<String, FuncSymbol>,
    pub(crate) static_func_files: HashMap<String, Vec<String>>,
    pub(crate) static_global_files: HashMap<String, Vec<String>>,
    pub(crate) structs: HashMap<String, StructSymbol>,
    pub(crate) unions: HashMap<String, StructSymbol>,
    pub(crate) classes: HashMap<String, ClassSymbol>,
    pub(crate) templates: HashMap<String, TemplateSymbol>,
    pub(crate) scopes: Vec<HashMap<String, VarSymbol>>,
    pub(crate) current_func_return: Type,
    pub(crate) current_file: String,
    pub(crate) loop_depth: i32,
    pub(crate) switch_depth: i32,
    pub(crate) current_func_params: HashSet<String>,
    pub(crate) func_labels: HashMap<String, SourceLoc>,
    pub(crate) pending_gotos: Vec<(String, SourceLoc)>,
    pub(crate) current_class: Option<String>,
    pub(crate) current_method_is_const: bool,
    /// Template instantiations discovered during type checking; appended to program.funcs at the end.
    pub(crate) pending_instantiations: Vec<(String, FuncDecl)>,
    /// Class template instantiations discovered during type checking; appended to program.classes at the end.
    pub(crate) pending_class_instantiations: Vec<(String, ClassDecl)>,
    /// U3#8：已见过的类模板实例化名（含已 drain 进 program.classes 的）——
    /// push 前查重的单源，防止重复实例化（第二个 vitro_vec<Foo> 的 E3002 假错误）。
    pub(crate) instantiated_class_names: std::collections::HashSet<String>,
    /// Lambdas discovered during type checking; lifted to ClassDecl + FuncDecl at the end.
    pub(crate) pending_lambdas: Vec<LambdaInfo>,
    /// W0-4（R-2026-09-12）：初始化器 char 窄化警告豁免开关。
    /// `'A'`（C 语义为 int 的字符常量）与值域在 char 内的整常量是 C 惯用写法，
    /// `char s[5]={72,101,...}` 曾逐元素报"可能丢失精度"误报轰炸合法代码。
    /// 由初始化检查路径（decl.rs / init.rs）在 `check_assignable` 前后成对置位/复位。
    pub(crate) char_narrow_suppress: bool,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self {
            is_library_mode: false,
            errors: Vec::new(),
            warnings: Vec::new(),
            hints: Vec::new(),
            funcs: HashMap::new(),
            static_func_sigs: HashMap::new(),
            static_func_files: HashMap::new(),
            static_global_files: HashMap::new(),
            structs: HashMap::new(),
            unions: HashMap::new(),
            classes: HashMap::new(),
            templates: HashMap::new(),
            scopes: Vec::new(),
            current_func_return: Type::void(),
            current_file: String::new(),
            loop_depth: 0,
            switch_depth: 0,
            current_func_params: HashSet::new(),
            func_labels: HashMap::new(),
            pending_gotos: Vec::new(),
            current_class: None,
            current_method_is_const: false,
            pending_instantiations: Vec::new(),
            pending_class_instantiations: Vec::new(),
            instantiated_class_names: std::collections::HashSet::new(),
            pending_lambdas: Vec::new(),
            char_narrow_suppress: false,
        }
    }
}

impl TypeChecker {
    pub fn new(is_library_mode: bool) -> Self {
        Self {
            is_library_mode,
            ..Self::default()
        }
    }

    pub fn check(mut self, program: &mut ProgramNode) -> (Vec<TypeError>, Vec<TypeError>, Vec<TypeError>) {
        // Pass 1: Register structs and unions
        for s in &program.structs {
            if self.structs.contains_key(&s.name) {
                self.report_error(
                    &format!("结构体 '{}' 重复定义", s.name),
                    &s.loc,
                    ErrorCode::E3002_StructRedeclared,
                );
                continue;
            }
            let sym = StructSymbol {
                fields: s.fields.iter().map(|f| (f.ty.clone(), f.name.clone())).collect(),
            };
            self.structs.insert(s.name.clone(), sym);
        }
        for u in &program.unions {
            if self.unions.contains_key(&u.name) {
                self.report_error(
                    &format!("联合体 '{}' 重复定义", u.name),
                    &u.loc,
                    ErrorCode::E3002_StructRedeclared,
                );
                continue;
            }
            let sym = StructSymbol {
                fields: u.fields.iter().map(|f| (f.ty.clone(), f.name.clone())).collect(),
            };
            self.unions.insert(u.name.clone(), sym);
        }

        // T-P0-8：struct/union 值成员循环包含检测（学生写链表节点漏 `*` 的经典
        // 错误，曾导致 compute_type_size 无限递归栈溢出、IDE 直接崩溃）。
        for s in &program.structs {
            if let Some(sym) = self.structs.get(&s.name) {
                let mut path = vec![s.name.clone()];
                if let Some(cycle) = sym.fields.iter().find_map(|(t, _)| self.value_member_cycle(t, &mut path)) {
                    self.report_error(
                        &format!(
                            "结构体 '{}' 的值成员存在循环包含（经 '{}' 回到自身）：结构体不能按值包含自己，链表/树节点请改用指针成员，如 `struct {}* next;`",
                            s.name, cycle, s.name
                        ),
                        &s.loc,
                        ErrorCode::E3072_StructSelfContain,
                    );
                }
            }
        }
        for u in &program.unions {
            if let Some(sym) = self.unions.get(&u.name) {
                let mut path = vec![u.name.clone()];
                if let Some(cycle) = sym.fields.iter().find_map(|(t, _)| self.value_member_cycle(t, &mut path)) {
                    self.report_error(
                        &format!(
                            "联合体 '{}' 的值成员存在循环包含（经 '{}' 回到自身），请改用指针成员",
                            u.name, cycle
                        ),
                        &u.loc,
                        ErrorCode::E3072_StructSelfContain,
                    );
                }
            }
        }

        // Pass 1.5: Register classes and compute layouts
        self.register_class_layouts(program);

        // Pass 1.6: Merge out-of-line method definitions into class declarations.
        // This handles C++ idiom `void Foo::bar() { ... }` and `Foo::Foo() { ... }`
        // by attaching the body to the in-class declaration, avoiding duplicate
        // function symbols and giving method bodies access to class fields.
        self.merge_out_of_line_method_definitions(program);

        // Pass 2: Register function signatures (including class methods as mangled funcs)
        for f in &program.funcs {
            let new_sym = FuncSymbol {
                return_type: f.return_type.clone(),
                param_types: f.params.iter().map(|p| p.ty.clone()).collect(),
                is_variadic: f.is_variadic,
                param_defaults: f.params.iter().map(|p| p.default.clone()).collect(),
            };
            if f.is_static {
                if let Some(existing) = self.static_func_sigs.get(&f.name) {
                    if existing.return_type != new_sym.return_type || existing.param_types != new_sym.param_types {
                        self.report_error(
                            &format!("函数 '{}' 的声明与之前定义签名不一致", f.name),
                            &f.loc,
                            ErrorCode::E3003_FuncRedeclared,
                        );
                    }
                } else {
                    self.static_func_sigs.insert(f.name.clone(), new_sym);
                }
                self.static_func_files
                    .entry(f.name.clone())
                    .or_default()
                    .push(f.source_file.clone());
            } else {
                if let Some(existing) = self.funcs.get(&f.name) {
                    if existing.return_type != new_sym.return_type || existing.param_types != new_sym.param_types {
                        self.report_error(
                            &format!("函数 '{}' 的声明与之前定义签名不一致", f.name),
                            &f.loc,
                            ErrorCode::E3003_FuncRedeclared,
                        );
                    }
                    continue;
                }
                self.funcs.insert(f.name.clone(), new_sym);
            }
        }

        // Class methods, constructors, and destructors are already registered as
        // mangled global function symbols by register_single_class_layout, including
        // overload-aware names (Class__method__N) when a class contains multiple
        // overloads of the same method.
        // Pass 2.4: Register class static fields as mangled global variables
        let mut static_field_globals: Vec<GlobalDecl> = Vec::new();
        for c in &program.classes {
            for member in &c.members {
                if let ClassMember::Field {
                    name: field_name,
                    ty,
                    is_static: true,
                    ..
                } = member
                {
                    let mangled = format!("{}__{}", c.name, field_name);
                    if !program.globals.iter().any(|g| g.name == mangled) {
                        static_field_globals.push(GlobalDecl {
                            loc: c.loc,
                            ty: ty.clone(),
                            name: mangled,
                            init: None,
                            is_static: false,
                            is_extern: false,
                            source_file: String::new(),
                        });
                    }
                }
            }
        }
        program.globals.extend(static_field_globals);

        // Pass 2.6: Register templates
        for t in &program.templates {
            let name = match &t.decl {
                Templateable::Func(f) => f.name.clone(),
                Templateable::Class(c) => c.name.clone(),
            };
            if self.templates.contains_key(&name) {
                self.report_error(&format!("模板 '{}' 重复定义", name), &t.loc, ErrorCode::E3003_FuncRedeclared);
                continue;
            }
            let sym = TemplateSymbol {
                params: t.params.clone(),
                decl: t.decl.clone(),
            };
            self.templates.insert(name, sym);
        }

        // Pass 2.65: Process explicit template class instantiations
        // (e.g. `template class vitro_vec<int>;`).
        for inst in &program.template_instantiations {
            if let Some((mangled, new_class)) = self.try_monomorphize_class(&inst.base, &inst.args) {
                if self.instantiated_class_names.insert(mangled.clone()) {
                    self.pending_class_instantiations.push((mangled, new_class));
                }
            } else if !self.templates.contains_key(&inst.base) {
                self.report_error(
                    &format!("未知模板类 '{}'", inst.base),
                    &inst.loc,
                    ErrorCode::E3023_UndeclaredVar,
                );
            }
        }

        // Pass 2.5: Register globals and check initializers
        self.enter_scope();
        // 条目 2（2026-09-11）：全局 `auto` / `typeof` 变量必须**先用初始化器定型再登记**。
        // 此前 `declare_var` 登记的是替换前的 `auto`，调用点查表得到 `auto` →
        // `auto gf = [](int x){ return x + 7; };` 的 `gf(1)` 报 E3066「不能对非函数类型进行调用」。
        // 初始化器在此只解析一次，结果缓存给下面的检查循环复用 —— 重复解析 lambda 会二次登记
        // `pending_lambdas`，进而在 Pass 4 重复生成 `__call` 定义。
        let mut precomputed_init_types: std::collections::HashMap<String, Type> = std::collections::HashMap::new();
        for g in &mut program.globals {
            if let Some(init) = g.init.as_mut() {
                if Self::type_has_auto(&g.ty) || Self::type_has_typeof(&g.ty) {
                    let init_ty = self.resolve_expr_type(init);
                    if Self::type_has_auto(&g.ty) {
                        g.ty = Self::replace_auto_in_type(&g.ty, init_ty.clone());
                    }
                    if Self::type_has_typeof(&g.ty) {
                        g.ty = Self::resolve_typeof_in_type(&g.ty, init_ty.clone());
                    }
                    precomputed_init_types.insert(g.name.clone(), init_ty);
                }
            }
        }
        for g in &mut program.globals {
            self.declare_var(&g.name, &g.ty, true, g.is_extern, g.is_static);
            if g.is_static {
                self.static_global_files
                    .entry(g.name.clone())
                    .or_default()
                    .push(g.source_file.clone());
            }
        }
        for g in &mut program.globals {
            // U1#12：全局数组尺寸合法性——extern 不完整声明（extern int g[];）
            // 是合法的，跳过；无初始化器的 `int a[];` 全局定义必须报错
            //（此前该路径完全无检查）
            if g.ty.is_array() && !g.is_extern {
                let has_init_list = matches!(g.init, Some(Expr::InitList { .. }) | Some(Expr::StringLiteral { .. }));
                let ty_snapshot = g.ty.clone();
                self.check_array_dims_legality(&ty_snapshot, has_init_list, &g.loc);
            }
            if let Some(ref mut init) = g.init {
                if g.ty.is_array() {
                    self.check_array_initializer(&mut g.ty, init, &g.loc);
                } else if g.ty.is_struct() && matches!(init, Expr::InitList { .. }) {
                    self.check_struct_initializer(&g.ty, init, &g.loc);
                } else {
                    let init_type = match precomputed_init_types.get(&g.name) {
                        Some(t) => t.clone(),
                        None => self.resolve_expr_type(init),
                    };
                    // 条目 2（2026-09-11）：全局 `auto` / `typeof` 变量此前**不做类型替换** ——
                    // 实测 `auto gf = [](int x){ return x + 7; };` 在文件作用域报
                    // E3004「无法将 'class __lambda_0' 赋值给 'auto'」（lambda 类型其实已推出）。
                    // 现与局部声明路径一致：先用初始化器类型替换声明类型，再判可赋值性。
                    if Self::type_has_auto(&g.ty) {
                        g.ty = Self::replace_auto_in_type(&g.ty, init_type.clone());
                    }
                    if Self::type_has_typeof(&g.ty) {
                        g.ty = Self::resolve_typeof_in_type(&g.ty, init_type.clone());
                    }
                    if !self.check_assignable(&g.ty, &init_type, &g.loc) {
                        self.report_error(
                            &format!("类型不匹配：无法将 '{}' 赋值给 '{}'", init_type, g.ty),
                            &g.loc,
                            ErrorCode::E3004_TypeMismatch,
                        );
                    }
                }
            }
        }

        // Pass 3: Check function bodies
        for f in &mut program.funcs {
            if f.body.is_some() {
                self.visit_func_decl(f);
            }
        }

        // Drain class template instantiations discovered during Pass 3
        // so Pass 3.5 can check their methods.
        let pending_classes: Vec<_> = std::mem::take(&mut self.pending_class_instantiations);
        for (_name, c) in pending_classes {
            program.classes.push(c);
        }

        // Pass 3.5: Check class method / constructor / destructor bodies
        self.check_class_methods(program);

        // Pass 3.55: Generate implicit move constructors for resource-holding classes
        self.generate_implicit_move_ctors(program);

        // Pass 3.6: Check pending function template instantiations recursively.
        // Function template instantiations discovered during Pass 3/3.5 may contain
        // further template calls (e.g. sort__int calls sort_rec__int), so we loop
        // until no new instantiations are generated.
        //
        // U3#5（T2）：类实例化与函数实例化**同收敛** drain——此前类只在 Pass 3
        // 后排空一次，本循环期间新发现的类（函数模板体内的 vitro_list<T> 等）
        // 被静默丢弃，合法 C++ 误拒（E3023/E3042 级联）。
        // U3#4（T1）：实例化轮数上限（对齐 clang -ftemplate-depth=1024）——
        // `template<class T> int f(T t){return f(&t);}` 每轮生成 f<T*>→f<T**>→…
        // 无限实例化直至 OOM（外部审查 3 秒栈溢出实锤）；超限确定性报错。
        const MAX_TEMPLATE_INSTANTIATION_ROUNDS: usize = 1024;
        let mut rounds = 0usize;
        while !self.pending_instantiations.is_empty() || !self.pending_class_instantiations.is_empty() {
            rounds += 1;
            if rounds > MAX_TEMPLATE_INSTANTIATION_ROUNDS {
                self.report_error(
                    &format!(
                        "模板实例化超过 {} 轮上限——存在无界实例化（如 f(&t) 自递归，每轮生成更深指针类型）。请检查模板递归终止条件。",
                        MAX_TEMPLATE_INSTANTIATION_ROUNDS
                    ),
                    &SourceLoc::default(),
                    ErrorCode::E1022_TemplateInstantiationLimit,
                );
                self.pending_instantiations.clear();
                self.pending_class_instantiations.clear();
                break;
            }
            let pending: Vec<_> = std::mem::take(&mut self.pending_instantiations);
            for (_, mut f) in pending {
                if f.body.is_some() {
                    self.visit_func_decl(&mut f);
                }
                program.funcs.push(f);
            }
            let pending_classes: Vec<_> = std::mem::take(&mut self.pending_class_instantiations);
            let got_new_class = !pending_classes.is_empty();
            for (_name, c) in pending_classes {
                program.classes.push(c);
            }
            if got_new_class {
                // 新类的方法体/构造析构需要检查（与 Pass 3.5 同语义）
                self.check_class_methods(program);
                self.generate_implicit_move_ctors(program);
            }
        }

        self.exit_scope();

        // Pass 4: Lift lambdas to ClassDecl + FuncDecl
        let lambdas: Vec<_> = std::mem::take(&mut self.pending_lambdas);
        for info in lambdas {
            let lambda_name = format!("__lambda_{}", info.id);
            let call_name = format!("{}__call", lambda_name);

            // Create ClassDecl with capture fields
            let class_members: Vec<ClassMember> = info
                .captures
                .iter()
                .map(|(name, ty, _)| ClassMember::Field {
                    name: name.clone(),
                    ty: ty.clone(),
                    access: AccessSpec::Public,
                    is_static: false,
                })
                .collect();
            program.classes.push(ClassDecl {
                loc: info.loc,
                name: lambda_name.clone(),
                base: None,
                members: class_members,
                vtable: None,
            });

            // Create FuncDecl for __call
            let mut call_params = vec![Param {
                name: "this".to_string(),
                ty: Type::Pointer {
                    pointee: Box::new(Type::Class {
                        name: lambda_name.clone(),
                        is_const: false,
                    }),
                    is_const: false,
                },
                loc: info.loc,
                default: None,
            }];
            call_params.extend(info.params.iter().cloned());

            let mut func_decl = FuncDecl {
                loc: info.loc,
                // 条目 1：与 resolve_lambda 注册的 __call 签名共用同一返回类型
                return_type: info.return_type.clone(),
                name: call_name,
                params: call_params,
                body: Some(info.body),
                is_static: false,
                is_extern: false,
                source_file: self.current_file.clone(),
                is_variadic: false,
            };

            // Rewrite capture variable accesses to this->field
            if let Some(ref mut body) = func_decl.body {
                Self::rewrite_lambda_captures(body, &info.captures, &lambda_name);
            }

            // Type-check the generated function body
            self.current_class = Some(lambda_name.clone());
            self.visit_func_decl(&mut func_decl);
            self.current_class = None;
            program.funcs.push(func_decl);
        }

        (self.errors, self.warnings, self.hints)
    }

    /// T-P0-8：沿"值语义 struct/union 成员"（含数组包裹）展开，检测循环包含。
    /// 返回路径中首先撞到的已存在节点名；指针/引用/函数成员不构成环。
    fn value_member_cycle(&self, ty: &Type, path: &mut Vec<String>) -> Option<String> {
        let mut ty_ref = ty;
        loop {
            match ty_ref {
                Type::Array { element, .. } => ty_ref = element,
                Type::Struct { name, .. } | Type::Union { name, .. } => {
                    if path.iter().any(|p| p == name) {
                        return Some(name.clone());
                    }
                    let fields: Option<&Vec<(Type, String)>> = self
                        .structs
                        .get(name)
                        .map(|s| &s.fields)
                        .or_else(|| self.unions.get(name).map(|s| &s.fields));
                    path.push(name.clone());
                    let hit = fields.and_then(|fs| fs.iter().find_map(|(t, _)| self.value_member_cycle(t, path)));
                    path.pop();
                    return hit;
                }
                _ => return None,
            }
        }
    }

    pub(crate) fn report_error(&mut self, msg: &str, loc: &SourceLoc, code: ErrorCode) {
        self.errors.push(TypeError {
            message: msg.to_string(),
            line: loc.line,
            column: loc.column,
            code: code as i32,
        });
    }

    pub(crate) fn report_warning(&mut self, msg: &str, loc: &SourceLoc, code: ErrorCode) {
        // W0-4：隐式标量转换警告按（行, 码）去重——初始化列表逐元素报同一条
        // "可能丢失精度"只会淹没真正有价值的诊断（同行第二条起丢弃）。
        if code == ErrorCode::W3053_ImplicitScalarConversion
            && self
                .warnings
                .iter()
                .any(|w| w.code == code as i32 && w.line == loc.line)
        {
            return;
        }
        self.warnings.push(TypeError {
            message: msg.to_string(),
            line: loc.line,
            column: loc.column,
            code: code as i32,
        });
    }

    /// U1#12（2026-09-13）：数组声明尺寸合法性——此前三形状零诊断静默接受
    /// （评估 C5/T6 实锤）：`int a[];`（无尺寸无初始化器，任何索引都是未知
    /// 边界越界）、`int b[-5];`（负尺寸）、`int c[0]={1,2};`（显式零尺寸，
    /// init.rs 曾把它静默推断成 2 元素数组）。clang 对三者均报错。
    ///
    /// 尺寸约定（parser `array_dim_info`）：`-1` = 未指定（哨兵）、`0` = VLA
    /// 或显式零（靠 `is_vla` 区分）、负字面量原样传入。**不误伤**：初始化器
    /// 推断（`int t[]={1,2}`，dims==-1 且有 InitList）、VLA（is_vla）、
    /// 函数参数退化（不经过本检查）、`extern` 不完整声明（调用方跳过）。
    pub(crate) fn check_array_dims_legality(&mut self, ty: &Type, has_init_list: bool, loc: &SourceLoc) {
        let Type::Array { dims, is_vla, .. } = ty else {
            return;
        };
        if *is_vla {
            return; // VLA 维度运行时求值，另有边界检查
        }
        let first = dims.first().copied().unwrap_or(-1);
        if first < -1 {
            self.report_error(
                &format!("数组大小不能为负数（{}）", first),
                loc,
                ErrorCode::E2002_ExpectedArraySize,
            );
        } else if first == -1 && !has_init_list {
            self.report_error(
                "数组缺少数组大小：需要显式大小或初始化器来推断（int a[N]; 或 int a[] = {...};）",
                loc,
                ErrorCode::E2002_ExpectedArraySize,
            );
        } else if first == 0 {
            // 显式零尺寸（`int c[0]`，含带初始化器——此前 {1,2} 被静默推断成
            // 2 元素数组）；零长度数组不是标准 C，教学子集不支持
            self.report_error(
                "数组大小不能为 0（零长度数组不在教学子集内）",
                loc,
                ErrorCode::E2002_ExpectedArraySize,
            );
        }
    }

    /// W0-4（R-2026-09-12）：int→char 初始化器是否确定无损。
    /// 字符常量（`'A'`，parser 记 `Literal{ty:char}`，C 语义提升为 int）与
    /// 值域在 char（-128..=127）内的整常量，赋给 char 变量/数组元素是 C
    /// 惯用写法，"可能丢失精度"属误报。
    pub(crate) fn is_char_safe_initializer(expr: &Expr) -> bool {
        match expr {
            Expr::Literal { value, ty, .. } => {
                ty.kind() == TypeKind::Char || (-128..=127).contains(value)
            }
            Expr::LongLiteral { value, .. } => (-128..=127).contains(value),
            _ => false,
        }
    }

    fn report_hint(&mut self, msg: &str, loc: &SourceLoc, code: ErrorCode) {
        self.hints.push(TypeError {
            message: msg.to_string(),
            line: loc.line,
            column: loc.column,
            code: code as i32,
        });
    }
}

mod builtin;
mod cpp;
mod cpp_auto;
mod cpp_class_layout;
mod cpp_container;
mod cpp_monomorph;
mod cpp_overload;
mod decl;
pub(crate) mod decl_types;
mod expr;

#[cfg(test)]
mod tests {
    use super::convert::implicit_cast_target;
    use super::*;

    fn loc() -> SourceLoc {
        SourceLoc::default()
    }

    #[test]
    fn test_implicit_cast_target_int_to_double() {
        assert_eq!(implicit_cast_target(&Type::int(), &Type::double()), Some(Type::double()));
    }

    #[test]
    fn test_implicit_cast_target_double_to_int() {
        assert_eq!(implicit_cast_target(&Type::double(), &Type::int()), Some(Type::int()));
    }

    #[test]
    fn test_implicit_cast_target_float_to_int() {
        assert_eq!(implicit_cast_target(&Type::float(), &Type::int()), Some(Type::int()));
    }

    #[test]
    fn test_implicit_cast_target_int_to_float() {
        assert_eq!(implicit_cast_target(&Type::int(), &Type::float()), Some(Type::float()));
    }

    #[test]
    fn test_implicit_cast_target_char_to_longlong() {
        assert_eq!(implicit_cast_target(&Type::char(), &Type::long_long()), Some(Type::long_long()));
    }

    #[test]
    fn test_implicit_cast_target_longlong_to_char() {
        assert_eq!(implicit_cast_target(&Type::long_long(), &Type::char()), Some(Type::char()));
    }

    #[test]
    fn test_implicit_cast_target_pointer_no_cast() {
        let p = Type::pointer_to(Type::int());
        assert_eq!(implicit_cast_target(&p, &Type::int()), None);
        assert_eq!(implicit_cast_target(&Type::int(), &p), None);
    }

    #[test]
    fn test_implicit_cast_target_reference_no_cast() {
        let r = Type::Reference {
            base: Box::new(Type::int()),
            is_const: false,
        };
        assert_eq!(implicit_cast_target(&r, &Type::double()), None);
        assert_eq!(implicit_cast_target(&Type::double(), &r), None);
    }

    #[test]
    fn test_insert_implicit_cast_int_literal_to_double() {
        let mut expr = Expr::Literal {
            value: 42,
            loc: loc(),
            ty: Type::int(),
        };
        insert_implicit_cast(&mut expr, &Type::double());
        assert!(matches!(
            expr,
            Expr::Cast {
                target_type: Type::Double { .. },
                ..
            }
        ));
    }

    #[test]
    fn test_insert_implicit_cast_float_literal_to_int() {
        let mut expr = Expr::FloatLiteral {
            value: 2.5,
            loc: loc(),
            ty: Type::float(),
        };
        insert_implicit_cast(&mut expr, &Type::int());
        assert!(matches!(
            expr,
            Expr::Cast {
                target_type: Type::Int { .. },
                ..
            }
        ));
    }
}
