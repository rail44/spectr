use crate::token::*;
use cranelift::prelude::*;
use cranelift_codegen::binemit::NullTrapSink;
use cranelift_codegen::Context;
use cranelift_codegen::ir::types::*;
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_simplejit::{SimpleJITBackend, SimpleJITBuilder};
use std::collections::HashMap;

pub fn compile(ast: &AST) -> *const u8 {
    let jit_builder = SimpleJITBuilder::new(cranelift_module::default_libcall_names());
    let mut module: Module<SimpleJITBackend> = Module::new(jit_builder);

    let mut scope_ctx = ScopeContext::new(module.make_context());

    let binds = HashMap::new();
    let mut translator = Translator::new(scope_ctx.get_builder(), binds, &mut module);
    let ret = translator.translate(&ast);

    translator.builder.ins().return_(&[ret]);
    translator.builder.finalize();

    let id = module
        .declare_function("top", Linkage::Export, &scope_ctx.ctx.func.signature)
        .map_err(|e| e.to_string())
        .unwrap();
    module
        .define_function(id, &mut scope_ctx.ctx, &mut NullTrapSink {})
        .unwrap();
    module.clear_context(&mut scope_ctx.ctx);
    module.finalize_definitions();
    module.get_finalized_function(id)
}

struct ScopeContext {
    pub ctx: Context,
    builder_context: FunctionBuilderContext,
}

impl ScopeContext {
    fn new(ctx: Context) -> ScopeContext {
        let builder_context = FunctionBuilderContext::new();
        ScopeContext {
            ctx,
            builder_context
        }
    }

    fn get_builder<'a>(&'a mut self) -> FunctionBuilder<'a> {
        self.ctx.func.signature.returns.push(AbiParam::new(F64));

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let block = builder.create_block();
        builder.append_block_params_for_function_params(block);
        builder.switch_to_block(block);
        builder.seal_block(block);
        builder
    }
}

struct Translator<'a> {
    binds: HashMap<String, FuncId>,
    builder: FunctionBuilder<'a>,
    module: &'a mut Module<SimpleJITBackend>,
}

impl<'a> Translator<'a> {
    fn new(
        builder: FunctionBuilder<'a>,
        binds: HashMap<String, FuncId>,
        module: &'a mut Module<SimpleJITBackend>,
    ) -> Translator<'a> {
        Translator {
            binds,
            builder,
            module,
        }
    }

    fn fork(&mut self, ctx: &'a mut ScopeContext) -> Translator {
        Translator::new(ctx.get_builder(), self.binds.clone(), self.module)
    }

    fn translate(&mut self, v: &Statement) -> Value {
        let mut b = Vec::new();

        for bind in v.definitions.iter() {
            let mut child_ctx = self.module.make_context();
            child_ctx.func.signature.returns.push(AbiParam::new(F64));
            let id = self
                .module
                .declare_function(
                    &format!("{}", bind.0),
                    Linkage::Local,
                    &child_ctx.func.signature,
                )
                .map_err(|e| e.to_string())
                .unwrap();
            self.binds.insert(bind.0.clone(), id);
            b.push((child_ctx, &bind.1, id));
        }

        for (mut child_ctx, body, id) in b {
            let mut builder_context = FunctionBuilderContext::new();
            let mut child_builder = FunctionBuilder::new(&mut child_ctx.func, &mut builder_context);
            let block = child_builder.create_block();
            child_builder.append_block_params_for_function_params(block);
            child_builder.switch_to_block(block);
            child_builder.seal_block(block);

            let mut translator =
                Translator::new(child_builder, self.binds.clone(), &mut self.module);
            let ret = translator.translate_additive(&body);
            translator.builder.ins().return_(&[ret]);
            translator.builder.finalize();
            self.module
                .define_function(id, &mut child_ctx, &mut NullTrapSink {})
                .unwrap();
        }

        self.translate_additive(&v.body)
    }

    fn translate_additive(&mut self, v: &Additive) -> Value {
        let mut lhs = self.translate_multitive(&v.left);
        for right in &v.rights {
            match right {
                AdditiveRight::Add(r) => {
                    let rhs = self.translate_multitive(&r);
                    lhs = self.builder.ins().fadd(lhs, rhs);
                }
                AdditiveRight::Sub(r) => {
                    let rhs = self.translate_multitive(&r);
                    lhs = self.builder.ins().fsub(lhs, rhs);
                }
            }
        }
        return lhs;
    }

    fn translate_multitive(&mut self, v: &Multitive) -> Value {
        let mut lhs = self.translate_primary(&v.left);
        for right in &v.rights {
            match right {
                MultitiveRight::Mul(r) => {
                    let rhs = self.translate_primary(&r);
                    lhs = self.builder.ins().fmul(lhs, rhs);
                }
                MultitiveRight::Div(r) => {
                    let rhs = self.translate_primary(&r);
                    lhs = self.builder.ins().fdiv(lhs, rhs);
                }
            }
        }
        lhs
    }

    fn translate_primary(&mut self, v: &Primary) -> Value {
        match v {
            Primary::Number(v) => self.builder.ins().f64const(v.clone()),
            Primary::Identifier(name) => {
                let id = self.binds.get(name).unwrap();
                let func = self.module.declare_func_in_func(*id, &mut self.builder.func);
                let call = self.builder.ins().call(func, &[]);
                self.builder.inst_results(call)[0]
            }
            _ => unimplemented!(),
        }
    }
}