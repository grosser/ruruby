use crate::vm::*;
use std::fs::File;
use std::io::Read;
use std::path::*;

pub fn init_file(globals: &mut Globals) -> Value {
    let id = globals.get_ident_id("File");
    let class = ClassRef::from(id, globals.object);
    let obj = Value::class(globals, class);
    globals.add_builtin_class_method(obj, "join", join);
    globals.add_builtin_class_method(obj, "basename", basename);
    globals.add_builtin_class_method(obj, "extname", extname);
    globals.add_builtin_class_method(obj, "binread", binread);
    obj
}

// Utils

fn string_to_path(vm: &mut VM, string: Value) -> Result<PathBuf, RubyError> {
    match string.as_string() {
        Some(file) => Ok(PathBuf::from(file)),
        None => Err(vm.error_type("Arg must be String.")),
    }
}

// Class methods

fn join(vm: &mut VM, args: &Args, _block: Option<MethodRef>) -> VMResult {
    vm.check_args_num(args.len(), 2, 2)?;
    let mut path = string_to_path(vm, args[0])?;
    let arg = string_to_path(vm, args[1])?;

    for p in arg.iter() {
        if p == ".." {
            path.pop();
        } else {
            path.push(p);
        }
    }
    Ok(Value::string(path.to_string_lossy().to_string()))
}

fn basename(vm: &mut VM, args: &Args, _block: Option<MethodRef>) -> VMResult {
    let len = args.len();
    vm.check_args_num(len, 1, 1)?;
    let filename = string_to_path(vm, args[0])?;
    let basename = match filename.file_name() {
        Some(ostr) => Value::string(ostr.to_string_lossy().into_owned()),
        None => Value::nil(),
    };
    Ok(basename)
}

fn extname(vm: &mut VM, args: &Args, _block: Option<MethodRef>) -> VMResult {
    let len = args.len();
    vm.check_args_num(len, 1, 1)?;
    let filename = string_to_path(vm, args[0])?;
    let extname = match filename.extension() {
        Some(ostr) => format!(".{}", ostr.to_string_lossy().into_owned()),
        None => "".to_string(),
    };
    Ok(Value::string(extname))
}

fn binread(vm: &mut VM, args: &Args, _block: Option<MethodRef>) -> VMResult {
    let len = args.len();
    vm.check_args_num(len, 1, 1)?;
    let filename = match string_to_path(vm, args[0])?.canonicalize() {
        Ok(file) => file,
        Err(_) => {
            return Err(vm.error_argument(format!("Invalid filename. {}", vm.val_pp(args[0]))))
        }
    };
    let mut file = match File::open(&filename) {
        Ok(file) => file,
        Err(_) => return Err(vm.error_internal(format!("Can not open file. {:?}", &filename))),
    };
    let mut contents = vec![];
    match file.read_to_end(&mut contents) {
        Ok(file) => file,
        Err(_) => return Err(vm.error_internal("Could not read the file.")),
    };
    Ok(Value::bytes(contents))
}