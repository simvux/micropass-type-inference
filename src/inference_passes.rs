use super::{
    ApplicationKey, Environment, GenericName, VariableInfo, VariableKey, VariableSource, record,
};
use log::{info, trace};
use std::ops::RangeTo;

#[derive(Clone)]
pub(crate) struct Application {
    pub(crate) func: VariableKey,
    pub(crate) parameters: Vec<VariableKey>,
    pub(crate) ret: VariableKey,
    pub(crate) satisfied: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct Assignment {
    pub(crate) lhs: VariableKey,
    pub(crate) rhs: VariableKey,
    pub(crate) satisfied: bool,
}

#[derive(Clone)]
pub(crate) struct SameasUnification {
    pub(crate) main: SameasMain,
    pub(crate) members: Vec<VariableKey>,
    pub(crate) satisfied: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum SameasMain {
    List { elem: VariableKey },
    JoinExpression(VariableKey),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct HasField {
    pub(crate) name: record::Field,
    pub(crate) field_type: VariableKey,
    pub(crate) instantiated_field_type: Option<VariableKey>,
    pub(crate) satisfied: bool,
}

type Pass = usize;

// Unify the type of a called function with its parameters where either side is known
const KNOWN_APPLICATIONS: Pass = 0;

// Unify the types from both sides of an assignment
const KNOWN_ASSIGNMENTS: Pass = 1;

// Unify the return of an inferred function type with what's expected at its call sites
const KNOWN_RETURN_TYPES: Pass = 2;

// Unify known types in lists and branching expressions with each other
const KNOWN_SAME_AS_UNIFICATIONS: Pass = 3;

// Unify the field types of known records with what's expected where those fields are accessed
const KNOWN_RECORD_FIELDS: Pass = 4;

// Find the type of a record from the name of its fields
const RESOLVE_RECORDS: Pass = 5;

// Infer into functions if the type variable is still unknown but is applied with parameters
const LESS_KNOWN_FUNCTIONS: Pass = 6;

// Infer numbers with unknown concrete types into a default numeric type
const DEFAULT_NUMBERS: Pass = 7;

// Infer unknown types into unit types or generics depending on whether they were defined in the
// function signature or function body.
const DEFAULT_UNKNOWNS_TO_UNIT_OR_LIFT: Pass = 8;

const ALL_PASSES: RangeTo<Pass> = ..DEFAULT_UNKNOWNS_TO_UNIT_OR_LIFT + 1;

/// Runs inference passes in order of user-relevance and repeats them recursively on previous type
/// variables as they may have changed.
///
/// Types are not really "checked" here. When a situation where not enough information is provided
/// is encountered, a pass will ignore and skip the scenario and wait for a future pass to re-run
/// self in the hopes that enough information will be known in the future.
///
/// The later the pass, the more aggressive changes its willing to make.
///
/// Once all passes have gotten the chance to run, all types are expected to have been inferred.
pub struct InferenceUnifier<'a> {
    env: &'a mut Environment,
    status: Status,
}

// To skip unecesarry unifications we track whether checks still has pending inference.
type Status = u8;
const STATUS_SATISFIED: Status = 0b00000000;
const STATUS_UNINFERRED: Status = 0b00000001;
const STATUS_CHANGED: Status = 0b00000010;

impl<'a> InferenceUnifier<'a> {
    pub fn new(env: &'a mut Environment) -> Self {
        Self {
            env,
            status: STATUS_SATISFIED,
        }
    }

    /// Perform complete type inference
    pub fn infer(&mut self) {
        self.perform(ALL_PASSES);
    }

    pub fn reset_and_is_satisfied(&mut self) -> bool {
        std::mem::take(&mut self.status) & STATUS_SATISFIED != 0
    }

    fn log_pass_status(&self, msg: impl std::fmt::Display) {
        if (self.status & STATUS_CHANGED) != 0 {
            info!("{msg} changed type variables")
        } else if (self.status & STATUS_UNINFERRED) != 0 {
            trace!("{msg} encountered uninferred type variables")
        }
    }

    fn perform(&mut self, passes: RangeTo<Pass>) {
        for pass in 0..passes.end {
            self.perform_pass(pass);
        }
    }

    fn perform_pass(&mut self, pass: Pass) {
        match pass {
            KNOWN_APPLICATIONS => self.known_applications(),
            KNOWN_ASSIGNMENTS => self.known_assignments(),
            KNOWN_RETURN_TYPES => self.known_return_types(),
            KNOWN_SAME_AS_UNIFICATIONS => self.known_same_as_unifications(),
            KNOWN_RECORD_FIELDS => self.known_record_fields(),
            RESOLVE_RECORDS => self.resolve_records(),
            LESS_KNOWN_FUNCTIONS => self.less_known_functions(),
            DEFAULT_NUMBERS => self.default_numbers(),
            DEFAULT_UNKNOWNS_TO_UNIT_OR_LIFT => self.default_unknown_to_unit_or_lift(),

            _ => panic!("unknown pass: {pass}"),
        }
    }

    fn known_applications(&mut self) {
        for appl_key in self.env.applications.keys() {
            let appl = &self.env.applications[appl_key];

            if appl.satisfied {
                continue;
            }

            let Some((parameters, _)) = self.env.as_function_cloned(appl.func) else {
                // Ignore applications against unknown functions as they may become known later
                continue;
            };

            if parameters.len() != appl.parameters.len() {
                // To prevent us accidentally ruining this functions inference by
                // unifying it with an invalid instantiation, we will skip this one.
                info!("parameter length differs, skipping {appl_key:?}");
                continue;
            }

            for (expected, given) in parameters.iter().zip(appl.parameters.clone()) {
                self.unify(*expected, given);
            }

            self.log_pass_status("known_applications");
            self.env.applications[appl_key].satisfied = self.reset_and_is_satisfied();
        }

        self.perform(..KNOWN_APPLICATIONS);
    }

    fn known_assignments(&mut self) {
        for i in 0..self.env.assignments.len() {
            let check = self.env.assignments[i];
            self.unify(check.lhs, check.rhs);

            self.log_pass_status("known_assignments");
            self.env.assignments[i].satisfied = self.reset_and_is_satisfied();
        }

        self.perform(..KNOWN_ASSIGNMENTS);
    }

    fn known_return_types(&mut self) {
        for appl_key in self.env.applications.keys() {
            let appl = &self.env.applications[appl_key];

            let Some((_, ret)) = self.env.as_function(appl.func) else {
                // Ignore applications against unknown functions as they may become known later
                continue;
            };

            self.unify(ret, appl.ret);

            self.log_pass_status("known_return_types");
            self.env.applications[appl_key].satisfied = self.reset_and_is_satisfied();
        }

        self.perform(..KNOWN_RETURN_TYPES);
    }

    fn known_same_as_unifications(&mut self) {
        for sameas_key in self.env.same_as_unifications.keys() {
            let expected = self.env.expected_of_sameas(sameas_key);

            for given in self.env.get_members(sameas_key) {
                self.unify(expected, given);
            }

            self.log_pass_status("known_same_as_unifications");
            self.env.same_as_unifications[sameas_key].satisfied = self.reset_and_is_satisfied();
        }

        self.perform(..KNOWN_SAME_AS_UNIFICATIONS)
    }

    fn known_record_fields(&mut self) {
        for var in self.env.vars() {
            let has_fields = &self.env.variables[var].has_fields;
            if has_fields.is_empty() || has_fields.iter().all(|f| f.satisfied) {
                continue;
            }

            let Some((name, params)) = self.env.as_record_cloned(var) else {
                continue;
            };

            for i in 0..has_fields.len() {
                let has_field = self.env.variables[var].has_fields[i];
                if has_field.satisfied {
                    continue;
                }

                let expected = match has_field.instantiated_field_type {
                    Some(ty) => ty,
                    // TODO: Does it makes more sense to store all the correct field types in the record
                    // when the VariableInfo::Record is created. That way we don't need to repeat
                    // the instantiation for each field.
                    None => match record::type_of_field(name, has_field.name) {
                        Ok(ty) => self.env.instantiate(&params, &ty),
                        Err(record::Error::RecordNotFound(_) | record::Error::FieldNotFound(_)) => {
                            break;
                        }
                    },
                };

                self.unify(expected, has_field.field_type);

                self.log_pass_status("known_record_fields");
                self.env.variables[var].has_fields[i].satisfied = self.reset_and_is_satisfied();
            }
        }

        self.perform(..KNOWN_RECORD_FIELDS)
    }

    fn resolve_records(&mut self) {
        for var in self.env.vars() {
            let var_data = &self.env.variables[var];

            let VariableInfo::Unknown = var_data.info else {
                continue;
            };

            if var_data.has_fields.is_empty() {
                continue;
            }

            if let Some(name) = record::guess_by_fields(var_data.has_fields.iter().map(|f| f.name))
            {
                info!("inferring {var} to be {name} because of its fields");
                let params = record::type_parameters(name, |_| self.env.unknown()).unwrap();
                self.env.variables[var].info = VariableInfo::Record(name, params);
            }
        }

        self.perform(..RESOLVE_RECORDS)
    }

    fn less_known_functions(&mut self) {
        for appl in self.env.applications.keys() {
            let func = self.env.applications[appl].func;

            match &self.env.variables[func].info {
                VariableInfo::Function { .. } => {}
                // If it's applied as a function but its type isn't known, then we assume it *is* a
                // function and infer it into a function.
                VariableInfo::Unknown => {
                    info!("inferring {func} to be function");

                    // Use the types from the first time the function is applied
                    let first = &self.env.applications[ApplicationKey(0)];

                    let info = VariableInfo::Function {
                        params: first.parameters.clone(),
                        ret: first.ret,
                    };

                    self.env.variables[func].info = info
                }
                _ => {}
            }

            self.log_pass_status("less_known_functions");
            self.reset_and_is_satisfied();
        }

        self.perform(..LESS_KNOWN_FUNCTIONS)
    }

    fn default_numbers(&mut self) {
        for var in self.env.vars() {
            if let VariableInfo::Numeric = &self.env.variables[var].info {
                info!("inferring {var} to be default int");
                self.env.variables[var].info = VariableInfo::default_int();
            }
        }

        self.perform(..DEFAULT_NUMBERS)
    }

    fn default_unknown_to_unit_or_lift(&mut self) {
        for var in self.env.vars() {
            let var_data = &mut self.env.variables[var];

            if let VariableInfo::Unknown = &var_data.info {
                let default_ = match var_data.source {
                    // If a type originating from the functions type signature is left unused, we declare a new
                    // generic to use instead of inferring unit.
                    VariableSource::Signature => {
                        let generic = self.implicitly_declare_generic();
                        info!("inferring {var} -> {generic}");
                        VariableInfo::Generic(generic)
                    }
                    VariableSource::Expression => {
                        info!("inferring {var} -> {{unit}}");
                        VariableInfo::default_unit_type()
                    }
                };

                self.env.variables[var].info = default_;
            }
        }
    }

    fn implicitly_declare_generic(&mut self) -> GenericName {
        const NAMES: &'static str = "abcdefghijklmnopqrstuvwxyz";

        for i in 0..NAMES.len() {
            let name = &NAMES[i..i + 1];

            if !self.env.generics.contains(name) {
                self.env.generics.insert(name);
                return name;
            }
        }

        panic!("ran out of implicit generic names");
    }

    fn unify(&mut self, expected: VariableKey, given: VariableKey) {
        trace!("unifying {expected} <> {given}");

        if expected == given {
            return;
        }

        let [exp_data, given_data] = self.env.variables.get_many_mut([expected, given]).unwrap();

        match [&mut exp_data.info, &mut given_data.info] {
            [VariableInfo::Int(exp_size), VariableInfo::Int(given_size)]
                if exp_size == given_size => {}
            [
                VariableInfo::Generic(exp_prim),
                VariableInfo::Generic(got_prim),
            ] if exp_prim == got_prim => {}
            [
                VariableInfo::Record(exp_ident, exp_params),
                VariableInfo::Record(given_ident, given_params),
            ] if given_ident == exp_ident => {
                let (exp_params, given_params) = (exp_params.clone(), given_params.clone());

                for (name, given) in given_params {
                    let expected = exp_params[name];
                    self.unify(expected, given);
                }
            }
            [
                VariableInfo::Function {
                    params: exp_params,
                    ret: exp_ret,
                },
                VariableInfo::Function { params, ret },
            ] if exp_params.len() == params.len() => {
                let (exp_params, params) = (exp_params.clone(), params.clone());
                let (exp_ret, ret) = (*exp_ret, *ret);

                for (exp_param, given_param) in exp_params.into_iter().zip(params) {
                    self.unify(exp_param, given_param);
                }
                self.unify(exp_ret, ret);
            }
            [VariableInfo::Numeric, VariableInfo::Numeric] => self.status |= STATUS_UNINFERRED,
            [VariableInfo::Numeric, VariableInfo::Int(size)] => {
                info!("inferring {expected} to be {size}");
                self.status |= STATUS_CHANGED;
                exp_data.info = VariableInfo::Int(*size);
            }
            [VariableInfo::Int(size), VariableInfo::Numeric] => {
                info!("inferring {given} to be {size}");
                self.status |= STATUS_CHANGED;
                given_data.info = VariableInfo::Int(*size);
            }
            [
                VariableInfo::Tuple(exp_elems),
                VariableInfo::Tuple(given_elems),
            ] => {
                let exp_elems = exp_elems.clone();
                let given_elems = given_elems.clone();

                for (exp_param, given_param) in exp_elems.into_iter().zip(given_elems) {
                    self.unify(exp_param, given_param);
                }
            }
            [
                VariableInfo::List(exp_inner),
                VariableInfo::List(given_inner),
            ] => {
                let [exp_inner, given_inner] = [*exp_inner, *given_inner];
                self.unify(exp_inner, given_inner)
            }

            // These may become known later, so let's leave it for now
            [VariableInfo::Unknown, VariableInfo::Unknown] => self.status |= STATUS_UNINFERRED,

            [VariableInfo::Unknown, _] => self.infer_directly(expected, given),
            [_, VariableInfo::Unknown] => self.infer_directly(given, expected),

            // These will fail the type checker.
            //
            // But if we were to create an error here, then that error would be re-created many
            // times.
            //
            // Type checking is instead performed on known types after type inference is entirely
            // finished.
            [_, _] => {}
        }
    }

    fn infer_directly(&mut self, unknown: VariableKey, known: VariableKey) {
        info!("inferring {unknown} =-> {:?}", &self.env.variables[known]);
        self.status |= STATUS_CHANGED;

        assert!(matches!(
            self.env.variables[unknown].info,
            VariableInfo::Unknown
        ));

        self.env.variables[unknown].info = self.env.variables[known].info.clone();
    }
}
