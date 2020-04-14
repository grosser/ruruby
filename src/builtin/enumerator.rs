use crate::*;

#[derive(Debug, Clone)]
pub struct EnumInfo {
    base: Value,
    method: IdentId,
    args: Args,
}

impl EnumInfo {
    pub fn new(base: Value, method: IdentId, args: Args) -> Self {
        EnumInfo { base, method, args }
    }
}

pub type EnumRef = Ref<EnumInfo>;

impl EnumRef {
    pub fn from(base: Value, method: IdentId, args: Args) -> Self {
        EnumRef::new(EnumInfo::new(base, method, args))
    }
}

pub fn init_enumerator(globals: &mut Globals) -> Value {
    let id = globals.get_ident_id("Enumerator");
    let class = ClassRef::from(id, globals.builtins.object);
    globals.add_builtin_instance_method(class, "each", each);
    globals.add_builtin_instance_method(class, "with_index", with_index);
    globals.add_builtin_instance_method(class, "inspect", inspect);
    let class = Value::class(globals, class);
    globals.add_builtin_class_method(class, "new", enum_new);
    class
}

// Class methods

fn enum_new(vm: &mut VM, args: &Args) -> VMResult {
    vm.check_args_num(args.len(), 1, 65535)?;
    let obj = args[0];
    let (method, new_args) = if args.len() == 1 {
        let method = vm.globals.get_ident_id("each");
        let new_args = Args::new0(args.self_value, None);
        (method, new_args)
    } else {
        if !args[1].is_packed_symbol() {
            return Err(vm.error_argument("2nd arg must be Symbol."));
        };
        let method = args[1].as_packed_symbol();
        let mut new_args = Args::new(args.len() - 2);
        for i in 0..args.len() - 2 {
            new_args[i] = args[i + 2];
        }
        new_args.self_value = args.self_value;
        new_args.block = None;
        (method, new_args)
    };
    let val = Value::enumerator(&vm.globals, obj, method, new_args);
    Ok(val)
}

// Instance methods

fn inspect(vm: &mut VM, args: &Args) -> VMResult {
    let eref = vm.expect_enumerator(args.self_value, "Expect Enumerator.")?;
    let inspect = format!(
        "#<Enumerator: {}:{}>",
        vm.val_inspect(eref.base),
        vm.globals.get_ident_name(eref.method)
    );
    Ok(Value::string(&vm.globals, inspect))
}

fn each(vm: &mut VM, args: &Args) -> VMResult {
    vm.check_args_num(args.len(), 0, 0)?;
    let eref = vm.expect_enumerator(args.self_value, "Expect Enumerator.")?;
    let block = match args.block {
        Some(method) => method,
        None => {
            return Ok(args.self_value);
        }
    };

    let receiver = eref.base;
    let each_method = vm.get_method(receiver, eref.method)?;
    let args = Args::new0(receiver, block);
    let val = vm.eval_send(each_method, &args)?;

    Ok(val)
}

fn with_index(vm: &mut VM, args: &Args) -> VMResult {
    vm.check_args_num(args.len(), 0, 0)?;
    let eref = vm.expect_enumerator(args.self_value, "Expect Enumerator.")?;
    let block = match args.block {
        Some(method) => method,
        None => {
            // return Enumerator
            let id = vm.globals.get_ident_id("with_index");
            let e = Value::enumerator(&vm.globals, args.self_value, id, args.clone());
            return Ok(e);
        }
    };

    let receiver = eref.base;
    let method = vm.get_method(receiver, eref.method)?;
    let mut args = eref.args.clone();
    args.block = Some(MethodRef::from(0));
    let val = vm.eval_send(method, &args)?;

    let ary = match val.as_array() {
        Some(ary) => ary,
        None => {
            let inspect = vm.val_inspect(val);
            return Err(vm.error_type(format!("Must be Array. {}", inspect)));
        }
    };

    if block == MethodRef::from(0) {
        let res_ary: Vec<Value> = ary
            .elements
            .iter()
            .enumerate()
            .map(|(i, v)| {
                Value::array(
                    &vm.globals,
                    ArrayRef::from(vec![v.clone(), Value::fixnum(i as i64)]),
                )
            })
            .collect();
        let res = Value::array(&vm.globals, ArrayRef::from(res_ary));
        eprintln!("{}", vm.val_inspect(res));
        return Ok(res);
    } else {
        let res_ary: Vec<(Value, Value)> = ary
            .elements
            .iter()
            .enumerate()
            .map(|(i, v)| (v.clone(), Value::fixnum(i as i64)))
            .collect();

        let mut res = vec![];
        let mut arg = Args::new(2);
        arg.self_value = vm.context().self_value;

        for (v, i) in &res_ary {
            arg[0] = v.clone();
            arg[1] = i.clone();
            let val = vm.eval_block(block, &arg)?;
            res.push(val);
        }

        let res = Value::array_from(&vm.globals, res);
        Ok(res)
    }
}

#[cfg(test)]
mod test {
    use crate::test::*;

    #[test]
    fn enumerator_with_index() {
        let program = r#"
        ans = %w(This is a Ruby.).map.with_index {|x| x }
        assert ["This", "is", "a", "Ruby."], ans
        ans = %w(This is a Ruby.).map.with_index {|x,y| [x,y] }
        assert [["This", 0], ["is", 1], ["a", 2], ["Ruby.", 3]], ans
        ans = %w(This is a Ruby.).map.with_index {|x,y,z| [x,y,z] }
        assert [["This", 0, nil], ["is", 1, nil], ["a", 2, nil], ["Ruby.", 3, nil]], ans
        "#;
        assert_script(program);
    }
}
