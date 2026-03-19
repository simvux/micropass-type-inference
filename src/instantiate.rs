use super::{Environment, Forall, GenericName, KnownType, VariableKey};

impl Environment {
    /// Instantiate a function
    pub fn instantiate_function(
        &mut self,
        generics: impl IntoIterator<Item = GenericName>,
        params: &[KnownType],
        ret: &KnownType,
    ) -> VariableKey {
        let annotation = generics
            .into_iter()
            .map(|name| (name, self.unknown()))
            .collect();

        let params = params
            .iter()
            .map(|ty| self.instantiate(&annotation, ty))
            .collect();

        let ret = self.instantiate(&annotation, ret);

        self.function(params, ret)
    }

    /// Convert a known type into a type variable in the environment while mapping generics to
    /// their annotated types.
    pub fn instantiate(&mut self, annotation: &Forall<VariableKey>, ty: &KnownType) -> VariableKey {
        let var = match ty {
            KnownType::Record(name, params) => {
                let params = params
                    .iter()
                    .map(|(name, ty)| (*name, self.instantiate(annotation, ty)))
                    .collect();

                self.record(*name, params)
            }
            KnownType::List(inner_type) => {
                let inner_type = self.instantiate(annotation, &inner_type);

                self.list(inner_type)
            }
            KnownType::Tuple(elems) => {
                let elems = elems
                    .iter()
                    .map(|ty| self.instantiate(annotation, ty))
                    .collect();

                self.tuple(elems)
            }
            KnownType::Generic(generic) => match annotation.get(*generic) {
                Some(ty) => *ty,
                None => panic!("{generic} not annotated"),
            },
            KnownType::String => self.string(),
            KnownType::Int(size) => self.int(*size),
            KnownType::Function { params, ret } => {
                let params = params
                    .iter()
                    .map(|ty| self.instantiate(annotation, ty))
                    .collect();

                let ret = self.instantiate(annotation, &ret);

                self.function(params, ret)
            }
        };

        log::info!("instantiated {ty} -> {var}");

        var
    }
}

/// Convert known types into other known types with generics mapped to the annotated types.
pub fn const_instantiate(annotation: &Forall<KnownType>, ty: &KnownType) -> KnownType {
    let new = match ty {
        KnownType::Record(name, forall) => {
            let forall = forall
                .iter()
                .map(|(generic, type_)| (*generic, const_instantiate(annotation, type_)))
                .collect();

            KnownType::Record(*name, forall)
        }
        KnownType::List(inner_type) => {
            let inner_type = const_instantiate(annotation, &**inner_type);
            KnownType::List(Box::new(inner_type))
        }
        KnownType::Tuple(elems) => {
            let elems = elems
                .iter()
                .map(|type_| const_instantiate(annotation, type_))
                .collect();

            KnownType::Tuple(elems)
        }
        KnownType::Generic(generic) => match annotation.get(*generic) {
            Some(ty) => ty.clone(),
            None => panic!("{generic} not annotated"),
        },
        KnownType::String => KnownType::String,
        KnownType::Int(size) => KnownType::Int(*size),
        KnownType::Function { params, ret } => {
            let params = params
                .iter()
                .map(|type_| const_instantiate(annotation, type_))
                .collect();

            let ret = Box::new(const_instantiate(annotation, ret));

            KnownType::Function { params, ret }
        }
    };

    log::info!("const instantiated {ty} -> {new}");

    new
}
