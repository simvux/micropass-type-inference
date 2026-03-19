use super::*;
use std::fmt;

/// Type check known types to generate the final error messages.
pub struct Checker<'a> {
    assignments: &'a Map<VariableKey, KnownType>,
    env: &'a Environment,

    errors: Vec<Error>,
}

#[derive(PartialEq, Eq, Clone)]
pub enum Error {
    Mismatch {
        expected: KnownType,
        given: KnownType,
        message: String,
    },
    NonFunctionApplication(KnownType),
    FunctionWrongParameterCount {
        expected: usize,
        given: usize,
    },
    DoesNotHaveFields(KnownType, Vec<record::Field>),
}

impl<'a> Checker<'a> {
    pub fn new(assignments: &'a Map<VariableKey, KnownType>, env: &'a Environment) -> Self {
        Self {
            assignments,
            env,
            errors: vec![],
        }
    }

    /// Type check known types to generate the final error messages.
    pub fn type_check(&mut self) -> Vec<Error> {
        for var in self.env.vars() {
            self.type_check_var(var);
        }

        for (sameas_key, sameas_unification) in self.env.same_as_unifications.iter() {
            let expected = &self.assignments[self.env.expected_of_sameas(sameas_key)];

            for var in &sameas_unification.members {
                let given = &self.assignments[*var];

                if *expected != *given {
                    let message = match sameas_unification.main {
                        SameasMain::List { .. } => {
                            "type must be same as the other types of this list"
                        }
                        SameasMain::ExpressionBranch(_) => {
                            "type must be same as the other branches of this expression"
                        }
                    };

                    self.err_type_mismatch(expected, given, message);
                }
            }
        }

        std::mem::take(&mut self.errors)
    }

    fn type_check_var(&mut self, var: VariableKey) {
        let variable = &self.env.variables[var];

        for appl in &variable.applied_by {
            self.type_check_application(var, *appl);
        }

        for dst in &variable.assigned_to {
            self.type_check_assignments(*dst, var);
        }

        if !variable.has_fields.is_empty() {
            self.type_check_fields(var);
        }
    }

    fn type_check_application(&mut self, var: VariableKey, appl: ApplicationKey) {
        let appl = &self.env.applications[appl];

        let KnownType::Function { params, ret } = &self.assignments[var] else {
            self.err(Error::NonFunctionApplication(self.assignments[var].clone()));
            return;
        };

        if params.len() != appl.parameters.len() {
            self.err(Error::FunctionWrongParameterCount {
                expected: params.len(),
                given: appl.parameters.len(),
            });

            return;
        }

        for (i, (expected, given_var)) in params.iter().zip(&appl.parameters).enumerate() {
            let given = &self.assignments[*given_var];
            if &*expected != given {
                self.err_type_mismatch(expected, given, format!("parameter {i} to this function"))
            }
        }

        if **ret != self.assignments[appl.ret] {
            self.err_type_mismatch(
                ret,
                &self.assignments[appl.ret],
                "return type of this function",
            )
        }
    }

    fn type_check_assignments(&mut self, dst: VariableKey, var: VariableKey) {
        let [expected, given] = [dst, var].map(|var| &self.assignments[var]);
        if expected != given {
            self.err_type_mismatch(expected, given, "can not be assigned to this type");
        }
    }

    fn type_check_fields(&mut self, var: VariableKey) {
        let variable = &self.env.variables[var];
        let type_ = &self.assignments[var];

        let KnownType::Record(name, forall) = type_ else {
            let fields = variable.has_fields.keys().copied().collect();
            self.err(Error::DoesNotHaveFields(type_.clone(), fields));
            return;
        };

        let unknown = variable
            .has_fields
            .iter()
            .filter_map(
                |(field, field_var)| match record::type_of_field(*name, *field) {
                    Ok(record_field_type) => {
                        let expected = const_instantiate(forall, &record_field_type);
                        let given = &self.assignments[*field_var];

                        if expected != *given {
                            self.err_type_mismatch(&expected, given, format!("field of {name}"));
                        }

                        None
                    }
                    Err(record::Error::RecordNotFound(record)) => {
                        panic!("KnownType points to a non-existent record {record}")
                    }
                    Err(record::Error::FieldNotFound(field)) => Some(field),
                },
            )
            .collect::<Vec<_>>();

        if !unknown.is_empty() {
            self.err(Error::DoesNotHaveFields(type_.clone(), unknown));
        }
    }

    fn err(&mut self, err: Error) {
        self.errors.push(err);
    }

    fn err_type_mismatch(
        &mut self,
        expected: &KnownType,
        given: &KnownType,
        message: impl Into<String>,
    ) {
        self.errors.push(Error::Mismatch {
            expected: expected.clone(),
            given: given.clone(),
            message: message.into(),
        });
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Mismatch {
                expected,
                given,
                message,
            } => write!(f, "expected {expected}, got {given}\n  {message}"),
            Error::NonFunctionApplication(called) => {
                write!(f, "cannot give parameters to non-function {called}")
            }
            Error::FunctionWrongParameterCount { expected, given } => write!(
                f,
                "function expected {expected} parameters, but was given {given}"
            ),
            Error::DoesNotHaveFields(type_, unknown) => {
                write!(f, "type {type_} does not have the fields ")?;

                for (i, name) in unknown.iter().enumerate() {
                    if i == 0 {
                        write!(f, "{name}")
                    } else {
                        write!(f, ", {name}")
                    }?;
                }

                Ok(())
            }
        }
    }
}
