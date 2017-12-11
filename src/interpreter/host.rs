use std::any::Any;
use std::sync::Arc;
use std::marker::PhantomData;
use std::collections::HashMap;
use elements::{FunctionType, ValueType, GlobalType, MemoryType, TableType};
use interpreter::store::{Store, ExternVal, ModuleId, ModuleInstance};
use interpreter::value::RuntimeValue;
use interpreter::Error;

enum HostItem {
	Func {
		name: String,
		func_type: FunctionType,
		host_func: Arc<AnyFunc>,
	},
	Global {
		name: String,
		global_type: GlobalType,
		init_val: RuntimeValue,
	},
	Memory {
		name: String,
		memory_type: MemoryType,
	},
	Table {
		name: String,
		table_type: TableType,
	}
}

pub struct HostModuleBuilder<St> {
	items: Vec<HostItem>,
	_marker: PhantomData<St>,
}

impl<St: 'static> HostModuleBuilder<St> {
	pub fn new() -> Self {
		HostModuleBuilder {
			items: Vec::new(),
			_marker: PhantomData,
		}
	}

	pub fn with_func1<
		Cl: Fn(&mut St, P1) -> Result<Option<Ret>, Error> + 'static,
		Ret: AsReturnVal + 'static,
		P1: FromArg + 'static,
		F: Into<Func1<Cl, St, Ret, P1>>,
	>(
		&mut self,
		name: &str,
		f: F,
	) {
		let func_type = Func1::<Cl, St, Ret, P1>::derive_func_type();
		let host_func = Arc::new(f.into()) as Arc<AnyFunc>;

		self.items.push(HostItem::Func {
			name: name.to_owned(),
			func_type,
			host_func,
		});
	}

	pub fn with_global(&mut self, name: &str, global_type: GlobalType, init_val: RuntimeValue) {
		self.items.push(HostItem::Global {
			name: name.to_owned(),
			global_type,
			init_val,
		});
	}

	pub fn with_memory(&mut self, name: &str, memory_type: MemoryType) {
		self.items.push(HostItem::Memory {
			name: name.to_owned(),
			memory_type,
		});
	}

	pub fn with_table(&mut self, name: &str, table_type: TableType) {
		self.items.push(HostItem::Table {
			name: name.to_owned(),
			table_type,
		});
	}

	pub fn build(self) -> HostModule {
		HostModule {
			items: self.items
		}
	}
}

pub struct HostModule {
	items: Vec<HostItem>,
}

impl HostModule {
	pub(crate) fn allocate(self, store: &mut Store) -> Result<ModuleId, Error> {
		let mut exports = HashMap::new();

		for item in self.items {
			match item {
				HostItem::Func { name, func_type, host_func } => {
					let type_id = store.alloc_func_type(func_type);
					let func_id = store.alloc_host_func(type_id, host_func);
					exports.insert(name, ExternVal::Func(func_id));
				},
				HostItem::Global { name, global_type, init_val } => {
					let global_id = store.alloc_global(global_type, init_val);
					exports.insert(name, ExternVal::Global(global_id));
				},
				HostItem::Memory { name, memory_type } => {
					let memory_id = store.alloc_memory(&memory_type)?;
					exports.insert(name, ExternVal::Memory(memory_id));
				},
				HostItem::Table { name, table_type } => {
					let table_id = store.alloc_table(&table_type)?;
					exports.insert(name, ExternVal::Table(table_id));
				}
			}
		}

		let host_module_instance = ModuleInstance::with_exports(exports);
		let module_id = store.add_module_instance(host_module_instance);

		Ok(module_id)
	}
}

pub trait AnyFunc {
	fn call_as_any(
		&self,
		state: &mut Any,
		args: &[RuntimeValue],
	) -> Result<Option<RuntimeValue>, Error>;
}

pub trait FromArg {
	fn from_arg(arg: &RuntimeValue) -> Self;
	fn value_type() -> ValueType;
}

impl FromArg for i32 {
	fn from_arg(arg: &RuntimeValue) -> Self {
		match arg {
			&RuntimeValue::I32(v) => v,
			unexpected => panic!("Expected I32, got {:?}", unexpected),
		}
	}

	fn value_type() -> ValueType {
		ValueType::I32
	}
}

pub trait AsReturnVal {
	fn as_return_val(self) -> Option<RuntimeValue>;
	fn value_type() -> Option<ValueType>;
}

impl AsReturnVal for i32 {
	fn as_return_val(self) -> Option<RuntimeValue> {
		Some(self.into())
	}

	fn value_type() -> Option<ValueType> {
		Some(ValueType::I32)
	}
}

impl AsReturnVal for () {
	fn as_return_val(self) -> Option<RuntimeValue> {
		None
	}

	fn value_type() -> Option<ValueType> {
		None
	}
}

pub struct Func1<Cl: Fn(&mut St, P1) -> Result<Option<Ret>, Error>, St, Ret: AsReturnVal, P1: FromArg> {
	closure: Cl,
	_marker: PhantomData<(St, Ret, P1)>,
}

impl<
	St: 'static,
	Ret: AsReturnVal,
	P1: FromArg,
	Cl: Fn(&mut St, P1) -> Result<Option<Ret>, Error>,
> AnyFunc for Func1<Cl, St, Ret, P1> {
	fn call_as_any(
		&self,
		state: &mut Any,
		args: &[RuntimeValue],
	) -> Result<Option<RuntimeValue>, Error> {
		let state = state.downcast_mut::<St>().unwrap();
		let p1 = P1::from_arg(&args[0]);
		let result = (self.closure)(state, p1);
		result.map(|r| r.and_then(|r| r.as_return_val()))
	}
}

impl<St: 'static, Ret: AsReturnVal, P1: FromArg, Cl: Fn(&mut St, P1) -> Result<Option<Ret>, Error>> From<Cl>
	for Func1<Cl, St, Ret, P1> {
	fn from(cl: Cl) -> Self {
		Func1 {
			closure: cl,
			_marker: PhantomData,
		}
	}
}

impl<
	St: 'static,
	Ret: AsReturnVal,
	P1: FromArg,
	Cl: Fn(&mut St, P1) -> Result<Option<Ret>, Error>,
> Func1<Cl, St, Ret, P1> {
	fn derive_func_type() -> FunctionType {
		FunctionType::new(vec![P1::value_type()], Ret::value_type())
	}
}
