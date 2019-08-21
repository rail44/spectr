use crate::types::Type;
use crate::{json, list, map, string, token, Env};
use std::cell::RefCell;
use std::iter::IntoIterator;
use std::rc::Rc;

pub fn eval_source(mut source: token::Source, env: &mut Env) -> Type {
    if let Some(expression) = source.expressions.pop() {
        source
            .binds
            .insert("List".to_string(), list::ListModule::get_value());
        source
            .binds
            .insert("Map".to_string(), map::MapModule::get_value());
        source
            .binds
            .insert("Json".to_string(), json::JsonModule::get_value());

        let mut env = Env {
            binds: source.binds,
            parent: Some(Rc::new(RefCell::new(env.clone()))),
        };
        return expression.eval(&mut env);
    }

    Type::Map(map::Map::new(env.clone(), source.binds))
}

impl Evaluable for token::Source {
    fn eval(self, env: &mut Env) -> Type {
        eval_source(self, env)
    }
}

pub trait Evaluable {
    fn eval(self, env: &mut Env) -> Type;
}

impl Evaluable for token::Expression {
    fn eval(self, env: &mut Env) -> Type {
        use token::Expression::*;
        match self {
            Comparison(c) => c.eval(env),
            Function(arg_names, expression) => Type::Function(
                env.clone(),
                arg_names,
                Box::new(Type::Unevaluated(*expression)),
            ),
            If(cond, cons, alt) => match cond.eval(env) {
                Type::Boolean(true) => cons.eval(env),
                Type::Boolean(false) => alt.eval(env),
                _ => panic!(),
            },
        }
    }
}

impl Evaluable for token::Comparison {
    fn eval(self, env: &mut Env) -> Type {
        let mut base = self.left.eval(env);

        for right in self.rights {
            use token::ComparisonKind::*;
            let value = right.value.eval(env);
            match right.kind {
                Equal => base = Type::Boolean(base == value),
                NotEqual => base = Type::Boolean(base != value),
            }
        }
        base
    }
}

impl Evaluable for token::Additive {
    fn eval(self, env: &mut Env) -> Type {
        let left = self.left.eval(env);

        if self.rights.is_empty() {
            return left;
        }

        if let Type::Number(mut base) = left {
            for right in self.rights {
                use token::AdditiveKind::*;
                if let Type::Number(value) = right.value.eval(env) {
                    match right.kind {
                        Add => base += value,
                        Sub => base -= value,
                    }
                    continue;
                }
                panic!("not a number");
            }
            return Type::Number(base);
        }
        panic!("not a number");
    }
}

impl Evaluable for token::Multitive {
    fn eval(self, env: &mut Env) -> Type {
        let left = self.left.clone().eval(env);

        if self.rights.is_empty() {
            return left;
        }

        if let Type::Number(mut base) = left {
            for right in self.rights {
                if let Type::Number(value) = right.value.clone().eval(env) {
                    use token::MultitiveKind::*;
                    match right.kind {
                        Mul => base *= value,
                        Div => base /= value,
                        Surplus => base %= value,
                    }
                    continue;
                }
                panic!("not a number: {:?}", right);
            }
            return Type::Number(base);
        }
        panic!("not a number: {:?}", self.left.clone());
    }
}

impl Evaluable for token::Primary {
    fn eval(mut self, env: &mut Env) -> Type {
        let mut base = self.0.remove(0).eval(env);

        for right in self.0 {
            if let token::Atom::Indentify(accessor) = right.base {
                base = base.get_prop(&accessor);

                for right in right.rights {
                    use token::PrimaryPartRight::*;
                    match right {
                        Indexing(arg) => match arg.eval(env) {
                            Type::String(s) => base = base.get_prop(&s),
                            Type::Number(n) => base = base.indexing(n as i32),
                            _ => panic!(),
                        },
                        Calling(expressions) => {
                            base =
                                base.call(expressions.into_iter().map(|e| e.eval(env)).collect());
                        }
                    }
                }
                continue;
            }
            panic!();
        }
        base
    }
}

impl Evaluable for token::PrimaryPart {
    fn eval(self, env: &mut Env) -> Type {
        let mut base = self.base.eval(env);

        for right in self.rights {
            use token::PrimaryPartRight::*;
            match right {
                Indexing(arg) => match arg.eval(env) {
                    Type::String(s) => base = base.get_prop(&s),
                    Type::Number(n) => base = base.indexing(n as i32),
                    _ => panic!(),
                },
                Calling(expressions) => {
                    base = base.call(expressions.into_iter().map(|e| e.eval(env)).collect());
                }
            }
        }
        base
    }
}

impl Evaluable for token::Atom {
    fn eval(self, env: &mut Env) -> Type {
        use token::Atom::*;
        match self {
            Number(f) => Type::Number(f),
            String(s) => Type::String(s),
            Parenthesis(a) => a.eval(env),
            Block(s) => s.eval(env),
            Null => Type::Null,
            Indentify(s) => env.get_value(&s),
            List(v) => Type::List(v.into_iter().map(|e| e.eval(env)).collect()),
        }
    }
}
