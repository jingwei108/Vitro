///|
/// E1 emitter 第三部分：decl 族（Param / FuncDecl / StructDecl /
/// GlobalDecl / Class 族 / Template 族）。
fn emit_param(p : Param, buf : StringBuilder) -> Unit {
  buf.write_string("{\"ty\":")
  emit_type(p.ty, buf)
  buf.write_string(",\"name\":")
  emit_json_str(p.name, buf)
  buf.write_string(",\"loc\":")
  emit_loc(p.loc, buf)
  buf.write_string(",\"default\":")
  emit_opt_expr(p.default, buf)
  buf.write_string("}")
}

///|
fn emit_func_decl(f : FuncDecl, buf : StringBuilder) -> Unit {
  buf.write_string("{\"loc\":")
  emit_loc(f.loc, buf)
  buf.write_string(",\"return_type\":")
  emit_type(f.return_type, buf)
  buf.write_string(",\"name\":")
  emit_json_str(f.name, buf)
  buf.write_string(",\"params\":")
  emit_array(f.params, buf, (p, b) => emit_param(p, b))
  buf.write_string(",\"body\":")
  emit_opt_stmt(f.body, buf)
  buf.write_string(",\"is_static\":")
  emit_bool(f.is_static, buf)
  buf.write_string(",\"is_extern\":")
  emit_bool(f.is_extern, buf)
  buf.write_string(",\"source_file\":")
  emit_json_str(f.source_file, buf)
  buf.write_string(",\"is_variadic\":")
  emit_bool(f.is_variadic, buf)
  buf.write_string("}")
}

///|
fn emit_struct_decl(d : StructDecl, buf : StringBuilder) -> Unit {
  buf.write_string("{\"loc\":")
  emit_loc(d.loc, buf)
  buf.write_string(",\"name\":")
  emit_json_str(d.name, buf)
  buf.write_string(",\"fields\":")
  emit_array(d.fields, buf, (f, b) => {
    b.write_string("{\"ty\":")
    emit_type(f.ty, b)
    b.write_string(",\"name\":")
    emit_json_str(f.name, b)
    b.write_string("}")
  })
  buf.write_string("}")
}

///|
fn emit_global_decl(g : GlobalDecl, buf : StringBuilder) -> Unit {
  buf.write_string("{\"loc\":")
  emit_loc(g.loc, buf)
  buf.write_string(",\"ty\":")
  emit_type(g.ty, buf)
  buf.write_string(",\"name\":")
  emit_json_str(g.name, buf)
  buf.write_string(",\"init\":")
  emit_opt_expr(g.init, buf)
  buf.write_string(",\"is_static\":")
  emit_bool(g.is_static, buf)
  buf.write_string(",\"is_extern\":")
  emit_bool(g.is_extern, buf)
  buf.write_string(",\"source_file\":")
  emit_json_str(g.source_file, buf)
  buf.write_string("}")
}

///|
fn emit_access_spec(a : AccessSpec, buf : StringBuilder) -> Unit {
  buf.write_string("\"")
  buf.write_string(
    match a {
      Public => "Public"
      Private => "Private"
      Protected => "Protected"
    },
  )
  buf.write_string("\"")
}

///|
fn emit_class_member(m : ClassMember, buf : StringBuilder) -> Unit {
  match m {
    Field(name, ty, access, is_static) => {
      buf.write_string("{\"Field\":{\"name\":")
      emit_json_str(name, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string(",\"access\":")
      emit_access_spec(access, buf)
      buf.write_string(",\"is_static\":")
      emit_bool(is_static, buf)
      buf.write_string("}}")
    }
    Method(name, ret, params, body, is_virtual, access, is_static, is_const) => {
      buf.write_string("{\"Method\":{\"name\":")
      emit_json_str(name, buf)
      buf.write_string(",\"ret\":")
      emit_type(ret, buf)
      buf.write_string(",\"params\":")
      emit_array(params, buf, (p, b) => emit_param(p, b))
      buf.write_string(",\"body\":")
      emit_opt_stmt(body, buf)
      buf.write_string(",\"is_virtual\":")
      emit_bool(is_virtual, buf)
      buf.write_string(",\"access\":")
      emit_access_spec(access, buf)
      buf.write_string(",\"is_static\":")
      emit_bool(is_static, buf)
      buf.write_string(",\"is_const\":")
      emit_bool(is_const, buf)
      buf.write_string("}}")
    }
    Constructor(params, body, is_default, access, is_explicit) => {
      buf.write_string("{\"Constructor\":{\"params\":")
      emit_array(params, buf, (p, b) => emit_param(p, b))
      buf.write_string(",\"body\":")
      emit_opt_stmt(body, buf)
      buf.write_string(",\"is_default\":")
      emit_bool(is_default, buf)
      buf.write_string(",\"access\":")
      emit_access_spec(access, buf)
      buf.write_string(",\"is_explicit\":")
      emit_bool(is_explicit, buf)
      buf.write_string("}}")
    }
    Destructor(body, access, is_virtual) => {
      buf.write_string("{\"Destructor\":{\"body\":")
      emit_opt_stmt(body, buf)
      buf.write_string(",\"access\":")
      emit_access_spec(access, buf)
      buf.write_string(",\"is_virtual\":")
      emit_bool(is_virtual, buf)
      buf.write_string("}}")
    }
    NestedStruct(decl, access) => {
      buf.write_string("{\"NestedStruct\":{\"decl\":")
      emit_struct_decl(decl, buf)
      buf.write_string(",\"access\":")
      emit_access_spec(access, buf)
      buf.write_string("}}")
    }
    NestedClass(decl, access) => {
      buf.write_string("{\"NestedClass\":{\"decl\":")
      emit_class_decl(decl, buf)
      buf.write_string(",\"access\":")
      emit_access_spec(access, buf)
      buf.write_string("}}")
    }
  }
}

///|
fn emit_class_decl(c : ClassDecl, buf : StringBuilder) -> Unit {
  buf.write_string("{\"loc\":")
  emit_loc(c.loc, buf)
  buf.write_string(",\"name\":")
  emit_json_str(c.name, buf)
  buf.write_string(",\"base\":")
  emit_opt_str(c.base, buf)
  buf.write_string(",\"members\":")
  emit_array(c.members, buf, (m, b) => emit_class_member(m, b))
  buf.write_string(",\"vtable\":")
  match c.vtable {
    Some(vt) => {
      buf.write_string("{\"entries\":[")
      for i, entry in vt.entries {
        if i > 0 {
          buf.write_string(",")
        }
        buf.write_string("[")
        emit_json_str(entry.0, buf)
        buf.write_string(",")
        emit_type(entry.1, buf)
        buf.write_string("]")
      }
      buf.write_string("]}")
    }
    None => buf.write_string("null")
  }
  buf.write_string("}")
}

///|
fn emit_template_param(p : TemplateParam, buf : StringBuilder) -> Unit {
  match p {
    TypeParam(name, loc) => {
      buf.write_string("{\"Type\":{\"name\":")
      emit_json_str(name, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
    NonType(name, ty, loc) => {
      buf.write_string("{\"NonType\":{\"name\":")
      emit_json_str(name, buf)
      buf.write_string(",\"ty\":")
      emit_type(ty, buf)
      buf.write_string(",\"loc\":")
      emit_loc(loc, buf)
      buf.write_string("}}")
    }
  }
}

///|
fn emit_templateable(t : Templateable, buf : StringBuilder) -> Unit {
  match t {
    TFunc(f) => {
      buf.write_string("{\"Func\":")
      emit_func_decl(f, buf)
      buf.write_string("}")
    }
    TClass(c) => {
      buf.write_string("{\"Class\":")
      emit_class_decl(c, buf)
      buf.write_string("}")
    }
  }
}

///|
fn emit_template_decl(t : TemplateDecl, buf : StringBuilder) -> Unit {
  buf.write_string("{\"loc\":")
  emit_loc(t.loc, buf)
  buf.write_string(",\"params\":")
  emit_array(t.params, buf, (p, b) => emit_template_param(p, b))
  buf.write_string(",\"decl\":")
  emit_templateable(t.decl, buf)
  buf.write_string("}")
}

///|
fn emit_template_inst(t : TemplateInstantiation, buf : StringBuilder) -> Unit {
  buf.write_string("{\"loc\":")
  emit_loc(t.loc, buf)
  buf.write_string(",\"base\":")
  emit_json_str(t.base, buf)
  buf.write_string(",\"args\":")
  emit_array(t.args, buf, (a, b) => emit_template_arg(a, b))
  buf.write_string("}")
}
