//! Helper methods for iterating and getting information from a type environment

use super::{
    Application, ApplicationKey, Environment, SameasMain, SameasUnificationKey, VariableInfo,
    VariableKey, record,
};
use std::collections::HashMap;

impl Environment {
    pub(crate) fn vars(&self) -> impl Iterator<Item = VariableKey> + 'static {
        self.variables.keys()
    }

    pub(crate) fn expected_of_sameas(&self, sameas_key: SameasUnificationKey) -> VariableKey {
        match self.same_as_unifications[sameas_key].main {
            SameasMain::List { elem, .. } => elem,
            SameasMain::ExpressionBranch(var) => var,
        }
    }

    pub(crate) fn get_applications(&self, func: VariableKey) -> Vec<(ApplicationKey, Application)> {
        self.variables[func]
            .applied_by
            .iter()
            .map(|appl| (*appl, self.applications[*appl].clone()))
            .collect()
    }

    pub(crate) fn get_members(&self, sameas: SameasUnificationKey) -> Vec<VariableKey> {
        self.same_as_unifications[sameas].members.clone()
    }

    pub(crate) fn get_assignments(&self, var: VariableKey) -> Vec<VariableKey> {
        self.variables[var].assigned_to.clone()
    }

    pub(crate) fn get_fields(&self, var: VariableKey) -> HashMap<record::Field, VariableKey> {
        self.variables[var].has_fields.clone()
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
}
