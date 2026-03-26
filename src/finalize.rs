use super::{Environment, KnownType, Map, VariableInfo, VariableKey};
use std::collections::HashMap;

/// Assume type inference has been completed and convert types to their known form.
pub struct Finalizer<'a> {
    assignments: HashMap<VariableKey, KnownType>,
    env: &'a mut Environment,
}

impl<'a> Finalizer<'a> {
    pub fn new(env: &'a mut Environment) -> Self {
        Self {
            assignments: HashMap::new(),
            env,
        }
    }

    pub fn finalize_all(&mut self) -> Map<VariableKey, KnownType> {
        let finalized = self
            .env
            .variables
            .keys()
            .map(|var| self.var_to_known(var))
            .collect();

        log::info!("finalized type environment:\n{finalized}");

        finalized
    }

    fn var(&mut self, var: VariableKey) {
        if self.assignments.contains_key(&var) {
            return;
        }

        let ty = match &self.env.variables[var].info {
            VariableInfo::Numeric | VariableInfo::Unknown => {
                panic!("type variable {var} remained un-inferred post type inference passes")
            }
            VariableInfo::Tuple(elems) => {
                let elems = elems
                    .clone()
                    .into_iter()
                    .map(|var| self.var_to_known(var))
                    .collect();

                KnownType::Tuple(elems)
            }
            VariableInfo::Record(name, params, _) => {
                let name = *name;
                let params = params
                    .clone()
                    .into_iter()
                    .map(|(generic, var)| (generic, self.var_to_known(var)))
                    .collect();

                KnownType::Record(name, params)
            }
            VariableInfo::List(inner_type) => {
                let inner_type = self.var_to_known(*inner_type);
                KnownType::List(Box::new(inner_type))
            }
            VariableInfo::Generic(generic) => KnownType::Generic(*generic),
            VariableInfo::String => KnownType::String,
            VariableInfo::Int(size) => KnownType::Int(*size),
            VariableInfo::Function { params, ret } => {
                let ret = *ret;
                let params = params
                    .clone()
                    .into_iter()
                    .map(|var| self.var_to_known(var))
                    .collect();
                let ret = self.var_to_known(ret);

                KnownType::Function {
                    params,
                    ret: Box::new(ret),
                }
            }
        };

        self.assignments.insert(var, ty);
    }

    fn var_to_known(&mut self, var: VariableKey) -> KnownType {
        self.var(var);
        self.assignments[&var].clone()
    }
}
