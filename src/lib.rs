use std::collections::{HashMap, HashSet};

mod checker;
mod finalize;
mod getters;
mod inference_passes;
mod instantiate;
mod record;
mod vecmap;

pub use checker::{Checker, Error};
pub use finalize::Finalizer;
pub use inference_passes::InferenceUnifier;
use instantiate::const_instantiate;
use std::fmt;
pub use vecmap::Map;

// Instead of type aliasing the indices to various Vec's we use statically typed keys
new_vec_key!(pub struct VariableKey, "var");
new_vec_key!(pub struct ApplicationKey, "application");
new_vec_key!(pub struct SameasUnificationKey, "same-as");

/// For illustrative purposes we use unrealistic static strings
pub type GenericName = &'static str;
pub type Forall<T> = HashMap<GenericName, T>;

/// The environment represents a single functions state as its in the process of being inferred and
/// type checked.
///
/// Each type that may not be fully known is represented as a type variable.
/// Type variables track information of what information about it is known, which functions its
/// applied to, what it's assigned to, etc.
pub struct Environment {
    variables: Map<VariableKey, Variable>,
    generics: HashSet<GenericName>,

    applications: Map<ApplicationKey, Application>,
    same_as_unifications: Map<SameasUnificationKey, SameasUnification>,

    current_source: VariableSource,
}

#[derive(Debug)]
struct Variable {
    info: VariableInfo,

    applied_by: Vec<ApplicationKey>,
    assigned_to: Vec<VariableKey>,
    has_fields: HashMap<record::Field, VariableKey>,

    source: VariableSource,
}

impl Variable {
    fn new(source: VariableSource, info: VariableInfo) -> Self {
        Self {
            info,
            applied_by: vec![],
            assigned_to: vec![],
            has_fields: HashMap::new(),

            source,
        }
    }
}

#[derive(Clone)]
struct Application {
    parameters: Vec<VariableKey>,
    ret: VariableKey,
}

struct SameasUnification {
    main: SameasMain,
    members: Vec<VariableKey>,
}

enum SameasMain {
    // All members must be identical and yield the type parameter of List
    List { elem: VariableKey },

    // All members must be identical and yield the Main
    ExpressionBranch(VariableKey),
}

/// The incomplete information we may hold of this variables inferred type
#[derive(Clone, Debug)]
enum VariableInfo {
    Unknown,
    Numeric,
    Record(record::Name, Forall<VariableKey>),
    Tuple(Vec<VariableKey>),
    List(VariableKey),
    Generic(GenericName),
    String,
    Int(Intsize),
    Function {
        params: Vec<VariableKey>,
        ret: VariableKey,
    },
}

impl VariableInfo {
    fn default_int() -> Self {
        VariableInfo::Int(Intsize::default())
    }

    fn default_unit_type() -> Self {
        VariableInfo::Tuple(vec![])
    }
}

/// Whether this variable was created in the functions type signature or elsewhere in its body.
///
/// The information is relevant for deciding how to default an uninferred type variable.
#[derive(Clone, Copy, Debug)]
enum VariableSource {
    Expression,
    Signature,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Intsize {
    bytes: u8,
}

impl Default for Intsize {
    fn default() -> Self {
        Self::from_bytes(4)
    }
}

impl Intsize {
    pub fn from_bits(bits: u8) -> Intsize {
        Intsize { bytes: bits / 8 }
    }

    pub fn from_bytes(bytes: u8) -> Intsize {
        Intsize { bytes }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum KnownType {
    Record(record::Name, Forall<Self>),
    List(Box<Self>),
    Tuple(Vec<Self>),
    Generic(GenericName),
    String,
    Int(Intsize),
    Function { params: Vec<Self>, ret: Box<Self> },
}

impl KnownType {
    pub fn record<const N: usize>(
        name: record::Name,
        params: [(GenericName, KnownType); N],
    ) -> Self {
        KnownType::Record(name, params.into())
    }

    pub fn list(inner: Self) -> Self {
        KnownType::List(Box::new(inner))
    }

    pub fn tuple<const N: usize>(elems: [Self; N]) -> Self {
        KnownType::Tuple(elems.into())
    }

    pub fn generic(generic: GenericName) -> Self {
        KnownType::Generic(generic)
    }

    pub fn string() -> Self {
        KnownType::String
    }

    pub fn function<const N: usize>(params: [Self; N], ret: Self) -> Self {
        KnownType::Function {
            params: params.into(),
            ret: Box::new(ret),
        }
    }

    pub fn i(bits: u8) -> Self {
        KnownType::Int(Intsize::from_bits(bits))
    }

    pub fn default_int() -> Self {
        KnownType::Int(Intsize::default())
    }

    pub fn default_unit_type() -> Self {
        KnownType::Tuple(vec![])
    }
}

impl Environment {
    pub fn new() -> Self {
        Self {
            variables: Map::new(),
            generics: HashSet::new(),

            same_as_unifications: Map::new(),
            applications: Map::new(),

            current_source: VariableSource::Signature,
        }
    }

    pub fn print_variables(&self) {
        println!("{:?}", &self.variables);
    }

    /// Marks the end of the function signatures.
    ///
    /// Type variables declared past this point will not be able to implicitly declare generics.
    pub fn leave_signature_enter_expression(&mut self) {
        self.current_source = VariableSource::Expression;
    }

    pub fn i(&mut self, bits: u8) -> VariableKey {
        self.variables.push(Variable::new(
            self.current_source,
            VariableInfo::Int(Intsize::from_bits(bits)),
        ))
    }

    pub fn int(&mut self, size: Intsize) -> VariableKey {
        self.variables
            .push(Variable::new(self.current_source, VariableInfo::Int(size)))
    }

    pub fn unknown(&mut self) -> VariableKey {
        self.variables
            .push(Variable::new(self.current_source, VariableInfo::Unknown))
    }

    pub fn numeric(&mut self) -> VariableKey {
        self.variables
            .push(Variable::new(self.current_source, VariableInfo::Numeric))
    }

    pub fn record(
        &mut self,
        name: record::Name,
        params: HashMap<GenericName, VariableKey>,
    ) -> VariableKey {
        self.variables.push(Variable::new(
            self.current_source,
            VariableInfo::Record(name, params),
        ))
    }

    pub fn tuple(&mut self, elements: Vec<VariableKey>) -> VariableKey {
        self.variables.push(Variable::new(
            self.current_source,
            VariableInfo::Tuple(elements),
        ))
    }

    /// Initialize a list where all members will inferred/checked to be the same type.
    pub fn list_sameas(&mut self) -> (SameasUnificationKey, VariableKey, VariableKey) {
        let elem = self.unknown();
        let var = self.list(elem);
        let key = self.same_as_unifications.push(SameasUnification {
            main: SameasMain::List { elem },
            members: vec![],
        });
        (key, var, elem)
    }

    /// Initialize a branching expression where all members will infer/check to be the type of the
    /// expression.
    pub fn expr_sameas(&mut self) -> (SameasUnificationKey, VariableKey) {
        let var = self.unknown();
        let key = self.same_as_unifications.push(SameasUnification {
            main: SameasMain::ExpressionBranch(var),
            members: vec![],
        });
        (key, var)
    }

    pub fn add_sameas_member(&mut self, sameas: SameasUnificationKey, var: VariableKey) {
        self.same_as_unifications[sameas].members.push(var);
    }

    pub fn add_field(&mut self, var: VariableKey, name: record::Field) -> VariableKey {
        let field_type = self.unknown();
        self.variables[var].has_fields.insert(name, field_type);
        field_type
    }

    pub fn list(&mut self, element_type: VariableKey) -> VariableKey {
        self.variables.push(Variable::new(
            self.current_source,
            VariableInfo::List(element_type),
        ))
    }

    pub fn generic(&mut self, generic_key: GenericName) -> VariableKey {
        self.variables.push(Variable::new(
            self.current_source,
            VariableInfo::Generic(generic_key),
        ))
    }

    pub fn string(&mut self) -> VariableKey {
        self.variables
            .push(Variable::new(self.current_source, VariableInfo::String))
    }

    pub fn function(&mut self, params: Vec<VariableKey>, ret: VariableKey) -> VariableKey {
        self.variables.push(Variable::new(
            self.current_source,
            VariableInfo::Function { params, ret },
        ))
    }

    /// Initialize function application.
    pub fn apply(&mut self, func: VariableKey) -> ApplicationKey {
        let ret = match &self.variables[func].info {
            // If it's known to be a function then use the known return type
            VariableInfo::Function { ret, .. } => *ret,
            // Otherwise generate a new return type and have that be checked later
            _ => self.unknown(),
        };

        let appl = self.applications.push(Application {
            parameters: vec![],
            ret,
        });

        self.variables[func].applied_by.push(appl);

        appl
    }

    /// Add the next parameter to the function application.
    pub fn apply_next_parameter(&mut self, appl: ApplicationKey, ty: VariableKey) {
        let appl = &mut self.applications[appl];
        appl.parameters.push(ty);
    }

    pub fn assign(&mut self, src: VariableKey, target: VariableKey) {
        self.variables[src].assigned_to.push(target);
    }

    /// Get the return type of a function application.
    pub fn get_return_type(&self, appl: ApplicationKey) -> VariableKey {
        self.applications[appl].ret
    }
}

impl fmt::Debug for KnownType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for KnownType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KnownType::Record(name, params) => {
                write!(f, "{}", name)?;
                if !params.is_empty() {
                    write!(f, " ")?;
                    let mut first = true;
                    for (_, param_type) in params {
                        if !first {
                            write!(f, " ")?;
                        }
                        first = false;
                        write!(f, "{}", param_type)?;
                    }
                }
                Ok(())
            }
            KnownType::List(elem_type) => write!(f, "[{}]", elem_type),
            KnownType::Tuple(elements) => {
                write!(f, "(")?;
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                write!(f, ")")
            }
            KnownType::Generic(name) => name.fmt(f),
            KnownType::String => "string".fmt(f),
            KnownType::Int(size) => size.fmt(f),
            KnownType::Function { params, ret } => {
                if params.is_empty() {
                    write!(f, "(() -> {})", ret)
                } else {
                    write!(f, "(")?;
                    for (i, param) in params.iter().enumerate() {
                        if i > 0 {
                            write!(f, " -> ")?;
                        }
                        write!(f, "{}", param)?;
                    }
                    write!(f, " -> {})", ret)
                }
            }
        }
    }
}

impl fmt::Display for Intsize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "i{}", self.bytes * 8)
    }
}

impl fmt::Debug for Intsize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "i{}", self.bytes * 8)
    }
}
