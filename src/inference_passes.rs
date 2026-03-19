use super::{Environment, GenericName, VariableInfo, VariableKey, VariableSource, record};
use std::ops::RangeTo;

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
}

impl<'a> InferenceUnifier<'a> {
    pub fn new(env: &'a mut Environment) -> Self {
        Self { env }
    }

    /// Perform complete type inference
    pub fn infer(&mut self) {
        self.perform(ALL_PASSES);
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
        for var in self.env.vars() {
            let Some((parameters, _)) = self.env.as_function_cloned(var) else {
                // Ignore applications against unknown functions as they may become known later
                continue;
            };

            for (appl_key, appl) in self.env.get_applications(var) {
                if parameters.len() != appl.parameters.len() {
                    // To prevent us accidentally ruining this functions inference by
                    // unifying it with an invalid instantiation, we will skip this one.
                    log::info!("parameter length differs, skipping {appl_key:?}");
                    continue;
                }

                for (expected, given) in parameters.iter().zip(appl.parameters) {
                    self.unify(*expected, given);
                }
            }
        }

        self.perform(..KNOWN_APPLICATIONS);
    }

    fn known_assignments(&mut self) {
        for var in self.env.vars() {
            for assigned in self.env.get_assignments(var) {
                self.unify(assigned, var);
            }
        }

        self.perform(..KNOWN_ASSIGNMENTS);
    }

    fn known_return_types(&mut self) {
        for var in self.env.vars() {
            let Some((_, ret)) = self.env.as_function(var) else {
                continue;
            };

            for (_, appl) in self.env.get_applications(var) {
                self.unify(ret, appl.ret);
            }
        }

        self.perform(..KNOWN_RETURN_TYPES);
    }

    fn known_same_as_unifications(&mut self) {
        for sameas_key in self.env.same_as_unifications.keys() {
            let expected = self.env.expected_of_sameas(sameas_key);

            for given in self.env.get_members(sameas_key) {
                self.unify(expected, given);
            }
        }

        self.perform(..KNOWN_SAME_AS_UNIFICATIONS)
    }

    fn known_record_fields(&mut self) {
        for var in self.env.vars() {
            let VariableInfo::Record(name, params) = &self.env.variables[var].info else {
                continue;
            };

            let name = *name;
            let params = params.clone();

            for (field_name, field_var) in self.env.get_fields(var) {
                match record::type_of_field(name, field_name) {
                    Ok(ty) => {
                        let expected = self.env.instantiate(&params, &ty);
                        log::info!(
                            "checking the field assignment {field_var} against instantiated field {expected}"
                        );
                        self.unify(expected, field_var);
                    }
                    Err(record::Error::RecordNotFound(_) | record::Error::FieldNotFound(_)) => {
                        break;
                    }
                }
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

            if let Some(name) = record::guess_by_fields(&var_data.has_fields) {
                log::info!("inferring {var} to be {name} because of its fields");
                let params = record::type_parameters(name, |_| self.env.unknown()).unwrap();
                self.env.variables[var].info = VariableInfo::Record(name, params);
            }
        }

        self.perform(..RESOLVE_RECORDS)
    }

    fn less_known_functions(&mut self) {
        for var in self.env.vars() {
            let is_applied = self.env.variables[var].applied_by.len() > 0;

            match &self.env.variables[var].info {
                VariableInfo::Function { .. } => {}
                // If it's applied as a function but its type isn't known, then we assume it *is* a
                // function and infer it into a function.
                VariableInfo::Unknown if is_applied => {
                    log::info!("inferring {var} to be function");

                    // Use the types from the first time the function is applied
                    let first_application_key = self.env.variables[var].applied_by[0];
                    let first_application = self.env.applications[first_application_key].clone();

                    self.env.variables[var].info = VariableInfo::Function {
                        params: first_application.parameters,
                        ret: first_application.ret,
                    };
                }
                _ => {}
            }
        }

        self.perform(..LESS_KNOWN_FUNCTIONS)
    }

    fn default_numbers(&mut self) {
        for var in self.env.vars() {
            if let VariableInfo::Numeric = &self.env.variables[var].info {
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
                        log::info!("inferring {var} -> {generic}");
                        VariableInfo::Generic(generic)
                    }
                    VariableSource::Expression => {
                        log::info!("inferring {var} -> {{unit}}");
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
        log::trace!("unifying {expected} <> {given}");

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
            [VariableInfo::Numeric, VariableInfo::Numeric] => {}
            [VariableInfo::Numeric, VariableInfo::Int(size)] => {
                log::info!("inferring {expected} to be {size}");
                exp_data.info = VariableInfo::Int(*size);
            }
            [VariableInfo::Int(size), VariableInfo::Numeric] => {
                log::info!("inferring {given} to be {size}");
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
            [VariableInfo::Unknown, VariableInfo::Unknown] => {}

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
        log::info!("inferring {unknown} =-> {known}");

        assert!(matches!(
            self.env.variables[unknown].info,
            VariableInfo::Unknown
        ));

        self.env.variables[unknown].info = self.env.variables[known].info.clone();
    }
}
