///|
/// E1 emitter 第二部分：Expr 26 / Stmt 16 / decl 族 / Designator /
/// InitElement / CaptureMode / AccessSpec（形态规格见 json_emit.mbt 包注释）。
fn emit_expr(e : Expr, buf : StringBuilder) -> Unit {
  match e {
    Binary(op, left, right, loc, ty) => {
      buf.write_string("{\"Binary\":{\"op\":")
      emit_binary_op(op, buf)
      buf.write_string(",\"left\":")
      emit_expr(left, buf)
      buf.write_string(",\"right\":")
      emit_expr(right, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Unary(op, operand, loc, ty) => {
      buf.write_string("{\"Unary\":{\"op\":")
      emit_unary_op(op, buf)
      buf.write_string(",\"operand\":")
      emit_expr(operand, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Literal(value, loc, ty) => {
      buf.write_string("{\"Literal\":{\"value\":\{value},\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    FloatLiteral(value, loc, ty) => {
      buf.write_string("{\"FloatLiteral\":{\"value\":")
      buf.write_string(double_to_json_text(value))
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    LongLiteral(value, loc, ty) => {
      buf.write_string("{\"LongLiteral\":{\"value\":\{value},\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    StringLiteral(value, loc, ty) => {
      buf.write_string("{\"StringLiteral\":{\"value\":")
      emit_json_str(value, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Identifier(name, loc, ty) => {
      buf.write_string("{\"Identifier\":{\"name\":")
      emit_json_str(name, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Call(name, args, loc, ty) => {
      buf.write_string("{\"Call\":{\"name\":")
      emit_json_str(name, buf)
      buf.write_string(",\"args\":")
      emit_array(args, buf, (a, b) => emit_expr(a, b))
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    CallPtr(callee, args, loc, ty) => {
      buf.write_string("{\"CallPtr\":{\"callee\":")
      emit_expr(callee, buf)
      buf.write_string(",\"args\":")
      emit_array(args, buf, (a, b) => emit_expr(a, b))
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Index(array, index, loc, ty) => {
      buf.write_string("{\"Index\":{\"array\":")
      emit_expr(array, buf)
      buf.write_string(",\"index\":")
      emit_expr(index, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Member(object, member_name, loc, ty) => {
      buf.write_string("{\"Member\":{\"object\":")
      emit_expr(object, buf)
      buf.write_string(",\"member\":")
      emit_json_str(member_name, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Assign(op, left, right, loc, ty) => {
      buf.write_string("{\"Assign\":{\"op\":")
      emit_assign_op(op, buf)
      buf.write_string(",\"left\":")
      emit_expr(left, buf)
      buf.write_string(",\"right\":")
      emit_expr(right, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Ternary(cond, then_branch, else_branch, loc, ty) => {
      buf.write_string("{\"Ternary\":{\"cond\":")
      emit_expr(cond, buf)
      buf.write_string(",\"then_branch\":")
      emit_expr(then_branch, buf)
      buf.write_string(",\"else_branch\":")
      emit_expr(else_branch, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Sizeof(target_type, operand, loc, ty) => {
      buf.write_string("{\"Sizeof\":{\"target_type\":")
      emit_opt_type(target_type, buf)
      buf.write_string(",\"operand\":")
      emit_opt_expr(operand, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Alignof(target_type, operand, loc, ty) => {
      buf.write_string("{\"Alignof\":{\"target_type\":")
      emit_opt_type(target_type, buf)
      buf.write_string(",\"operand\":")
      emit_opt_expr(operand, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Cast(expr, target_type, loc, ty) => {
      buf.write_string("{\"Cast\":{\"expr\":")
      emit_expr(expr, buf)
      buf.write_string(",\"target_type\":")
      emit_type(target_type, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    InitList(elements, loc, ty) => {
      buf.write_string("{\"InitList\":{\"elements\":")
      emit_array(elements, buf, (el, b) => {
        b.write_string("{\"designators\":")
        emit_array(el.designators, b, (d, bb) => emit_designator(d, bb))
        b.write_string(",\"value\":")
        emit_expr(el.value, b)
        b.write_string("}")
      })
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Offsetof(target_type, field, loc, ty) => {
      buf.write_string("{\"Offsetof\":{\"target_type\":")
      emit_type(target_type, buf)
      buf.write_string(",\"field\":")
      emit_json_str(field, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Generic(control, associations, default, loc, ty) => {
      buf.write_string("{\"Generic\":{\"control\":")
      emit_expr(control, buf)
      buf.write_string(",\"associations\":")
      emit_array(associations, buf, (assoc, b) => {
        b.write_string("[")
        emit_type(assoc.0, b)
        b.write_string(",")
        emit_expr(assoc.1, b)
        b.write_string("]")
      })
      buf.write_string(",\"default\":")
      emit_opt_expr(default, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    CompoundLiteral(target_type, init, loc, ty) => {
      buf.write_string("{\"CompoundLiteral\":{\"target_type\":")
      emit_type(target_type, buf)
      buf.write_string(",\"init\":")
      emit_expr(init, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    This(loc, ty) => {
      buf.write_string("{\"This\":{\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    MemberCall(object, method_name, args, is_virtual, resolved_mangled, loc, ty) => {
      buf.write_string("{\"MemberCall\":{\"object\":")
      emit_expr(object, buf)
      buf.write_string(",\"method\":")
      emit_json_str(method_name, buf)
      buf.write_string(",\"args\":")
      emit_array(args, buf, (a, b) => emit_expr(a, b))
      buf.write_string(",\"is_virtual\":")
      emit_bool(is_virtual, buf)
      buf.write_string(",\"resolved_mangled\":")
      match resolved_mangled {
        Some(m) => emit_json_str(m, buf)
        None => buf.write_string("null")
      }
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    New(elem_type, size_expr, init, loc, ty) => {
      buf.write_string("{\"New\":{\"elem_type\":")
      emit_type(elem_type, buf)
      buf.write_string(",\"size_expr\":")
      emit_opt_expr(size_expr, buf)
      buf.write_string(",\"init\":")
      emit_opt_expr(init, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Delete(expr, is_array, loc, ty) => {
      buf.write_string("{\"Delete\":{\"expr\":")
      emit_expr(expr, buf)
      buf.write_string(",\"is_array\":")
      emit_bool(is_array, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Lambda(capture, params, body, unique_id, loc, ty) => {
      buf.write_string("{\"Lambda\":{\"capture\":")
      emit_array(capture, buf, (c, b) => emit_capture_mode(c, b))
      buf.write_string(",\"params\":")
      emit_array(params, buf, (p, b) => emit_param(p, b))
      buf.write_string(",\"body\":")
      emit_stmt(body, buf)
      buf.write_string(",\"unique_id\":\{unique_id},\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
    Move(expr, loc, ty) => {
      buf.write_string("{\"Move\":{\"expr\":")
      emit_expr(expr, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string("}}")
    }
  }
}

///|
fn emit_designator(d : Designator, buf : StringBuilder) -> Unit {
  match d {
    Field(name) => {
      buf.write_string("{\"Field\":")
      emit_json_str(name, buf)
      buf.write_string("}")
    }
    Index(e) => {
      buf.write_string("{\"Index\":")
      emit_expr(e, buf)
      buf.write_string("}")
    }
  }
}

///|
fn emit_capture_mode(c : CaptureMode, buf : StringBuilder) -> Unit {
  match c {
    ByValue(name) => {
      buf.write_string("{\"ByValue\":")
      emit_json_str(name, buf)
      buf.write_string("}")
    }
    ByReference(name) => {
      buf.write_string("{\"ByReference\":")
      emit_json_str(name, buf)
      buf.write_string("}")
    }
    Implicit => buf.write_string("\"Implicit\"")
  }
}

///|
fn emit_opt_type(t : Type?, buf : StringBuilder) -> Unit {
  match t {
    Some(x) => emit_type(x, buf)
    None => buf.write_string("null")
  }
}

///|
fn emit_opt_expr(e : Expr?, buf : StringBuilder) -> Unit {
  match e {
    Some(x) => emit_expr(x, buf)
    None => buf.write_string("null")
  }
}

///|
fn emit_opt_stmt(s : Stmt?, buf : StringBuilder) -> Unit {
  match s {
    Some(x) => emit_stmt(x, buf)
    None => buf.write_string("null")
  }
}

///|
fn emit_opt_str(s : String?, buf : StringBuilder) -> Unit {
  match s {
    Some(x) => emit_json_str(x, buf)
    None => buf.write_string("null")
  }
}

// ---- Stmt ----

///|
fn emit_stmt(s : Stmt, buf : StringBuilder) -> Unit {
  match s {
    Block(stmts, loc) => {
      buf.write_string("{\"Block\":{\"stmts\":")
      emit_array(stmts, buf, (st, b) => emit_stmt(st, b))
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    VarDecl(var_type, name, init, extra_vars, is_static, loc) => {
      buf.write_string("{\"VarDecl\":{\"var_type\":")
      emit_type(var_type, buf)
      buf.write_string(",\"name\":")
      emit_json_str(name, buf)
      buf.write_string(",\"init\":")
      emit_opt_expr(init, buf)
      // extra_vars：Rust 侧 (Type, String, Option<Expr>) 元组 → JSON 数组
      buf.write_string(",\"extra_vars\":[")
      for i, extra in extra_vars {
        if i > 0 {
          buf.write_string(",")
        }
        buf.write_string("[")
        emit_type(extra.var_type, buf)
        buf.write_string(",")
        emit_json_str(extra.name, buf)
        buf.write_string(",")
        emit_opt_expr(extra.init, buf)
        buf.write_string("]")
      }
      buf.write_string("],\"is_static\":")
      emit_bool(is_static, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    Expr(expr, loc) => {
      buf.write_string("{\"Expr\":{\"expr\":")
      emit_expr(expr, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    If(cond, then_stmt, else_stmt, loc) => {
      buf.write_string("{\"If\":{\"cond\":")
      emit_expr(cond, buf)
      buf.write_string(",\"then_stmt\":")
      emit_stmt(then_stmt, buf)
      buf.write_string(",\"else_stmt\":")
      emit_opt_stmt(else_stmt, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    While(cond, body, loc) => {
      buf.write_string("{\"While\":{\"cond\":")
      emit_expr(cond, buf)
      buf.write_string(",\"body\":")
      emit_stmt(body, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    DoWhile(body, cond, loc) => {
      buf.write_string("{\"DoWhile\":{\"body\":")
      emit_stmt(body, buf)
      buf.write_string(",\"cond\":")
      emit_expr(cond, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    For(init, cond, step, body, loc) => {
      buf.write_string("{\"For\":{\"init\":")
      emit_opt_stmt(init, buf)
      buf.write_string(",\"cond\":")
      emit_opt_expr(cond, buf)
      buf.write_string(",\"step\":")
      emit_array(step, buf, (e, b) => emit_expr(e, b))
      buf.write_string(",\"body\":")
      emit_stmt(body, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    Return(value, loc) => {
      buf.write_string("{\"Return\":{\"value\":")
      emit_opt_expr(value, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    Break(loc) => {
      buf.write_string("{\"Break\":{\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    Continue(loc) => {
      buf.write_string("{\"Continue\":{\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    Switch(cond, body, loc) => {
      buf.write_string("{\"Switch\":{\"cond\":")
      emit_expr(cond, buf)
      buf.write_string(",\"body\":")
      emit_stmt(body, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    Case(label, stmt, loc) => {
      buf.write_string("{\"Case\":{\"label\":")
      emit_opt_expr(label, buf)
      buf.write_string(",\"stmt\":")
      emit_stmt(stmt, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    Goto(label, loc) => {
      buf.write_string("{\"Goto\":{\"label\":")
      emit_json_str(label, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    Label(label, stmt, loc) => {
      buf.write_string("{\"Label\":{\"label\":")
      emit_json_str(label, buf)
      buf.write_string(",\"stmt\":")
      emit_stmt(stmt, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    RangeFor(v, var_type, iter, body, loc) => {
      buf.write_string("{\"RangeFor\":{\"var\":")
      emit_json_str(v, buf)
      buf.write_string(",\"var_type\":")
      emit_type(var_type, buf)
      buf.write_string(",\"iter\":")
      emit_expr(iter, buf)
      buf.write_string(",\"body\":")
      emit_stmt(body, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    Try(body, catches, loc) => {
      buf.write_string("{\"Try\":{\"body\":")
      emit_stmt(body, buf)
      buf.write_string(",\"catches\":")
      emit_array(catches, buf, (c, b) => {
        b.write_string("{\"param_type\":")
        emit_type(c.param_type, b)
        b.write_string(",\"param_name\":")
        emit_opt_str(c.param_name, b)
        b.write_string(",\"body\":")
        emit_stmt(c.body, b)
        b.write_string(",\"loc\":")
        emit_loc(c.loc, b)
        b.write_string("}")
      })
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
  }
}
