//! Helper methods for iterating and getting information from a type environment

use super::{Environment, SameasUnificationKey, VariableInfo, VariableKey, inf, record};
use std::collections::HashMap;

impl Environment {
    pub(crate) fn vars(&self) -> impl Iterator<Item = VariableKey> + 'static {
        self.variables.keys()
    }

    pub(crate) fn expected_of_sameas(&self, sameas_key: SameasUnificationKey) -> VariableKey {
        match self.same_as_unifications[sameas_key].main {
            inf::SameasMain::List { elem, .. } => elem,
            inf::SameasMain::JoinExpression(var) => var,
        }
    }

    pub(crate) fn get_members(&self, sameas: SameasUnificationKey) -> Vec<VariableKey> {
        self.same_as_unifications[sameas].members.clone()
    }

    pub(crate) fn as_function(&self, var: VariableKey) -> Option<(&[VariableKey], VariableKey)> {
        match &self.variables[var].info {
            VariableInfo::Function { params, ret } => Some((params, *ret)),
            _ => None,
        }
    }

    pub(crate) fn as_function_cloned(
        &self,
        var: VariableKey,
    ) -> Option<(Vec<VariableKey>, VariableKey)> {
        match &self.variables[var].info {
            VariableInfo::Function { params, ret } => Some((params.to_vec(), *ret)),
            _ => None,
        }
    }

    pub(crate) fn get_record_fields(
        &self,
        var: VariableKey,
    ) -> Option<HashMap<record::Field, VariableKey>> {
        match &self.variables[var].info {
            VariableInfo::Record(_, _, fields) => Some(fields.clone()),
            _ => None,
        }
    }
}
